//! In-app updater: asks GitHub Releases for a newer version, downloads the
//! matching asset (verifying its SHA-256) and applies it.
//!
//! Everything the updater creates lives in `%TEMP%\cube-update\` or next to
//! the executable as `<exe>.old` / `<exe>.new`, and [`cleanup`] removes all of
//! it on every startup.

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

const LATEST_URL: &str = "https://api.github.com/repos/enekocm7/cube-app/releases/latest";
const RELEASES_URL: &str = "https://api.github.com/repos/enekocm7/cube-app/releases?per_page=20";
const UPDATE_DIR: &str = "cube-update";

/// Version of the running binary.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn user_agent() -> String {
    format!("cube/{CURRENT_VERSION}")
}

// ----------------------------------------------------------------------
// Errors
// ----------------------------------------------------------------------

#[derive(Debug)]
pub enum UpdateError {
    /// No network (DNS failure, connection refused, timeout…).
    Offline,
    /// GitHub answered 403 / 429.
    RateLimited,
    Cancelled,
    HashMismatch,
    Other(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offline => f.write_str("no internet connection"),
            Self::RateLimited => f.write_str("GitHub rate limit reached, try again later"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::HashMismatch => {
                f.write_str("the downloaded file is corrupted (SHA-256 mismatch) and was deleted")
            }
            Self::Other(msg) => f.write_str(msg),
        }
    }
}

impl From<std::io::Error> for UpdateError {
    fn from(error: std::io::Error) -> Self {
        Self::Other(error.to_string())
    }
}

impl From<ureq::Error> for UpdateError {
    fn from(error: ureq::Error) -> Self {
        match error {
            ureq::Error::StatusCode(403 | 429) => Self::RateLimited,
            ureq::Error::StatusCode(404) => Self::Other("release not found".into()),
            ureq::Error::StatusCode(code) => Self::Other(format!("GitHub returned HTTP {code}")),
            ureq::Error::Io(_)
            | ureq::Error::Timeout(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed => Self::Offline,
            other => Self::Other(other.to_string()),
        }
    }
}

// ----------------------------------------------------------------------
// Release metadata
// ----------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: Version,
    pub notes: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
}

/// `v1.2.3` / `1.2.3` -> `Version`.
pub fn parse_version(tag: &str) -> Option<Version> {
    Version::parse(tag.trim().trim_start_matches(['v', 'V'])).ok()
}

/// True when `candidate` is strictly newer than `current`.
pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (parse_version(current), parse_version(candidate)) {
        (Some(current), Some(candidate)) => candidate > current,
        _ => false,
    }
}

/// Which releases the update check looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Channel {
    /// Finished releases only (GitHub's `releases/latest`).
    #[default]
    Stable,
    /// Also offer pre-releases (the newest tag wins, drafts excluded).
    Prerelease,
}

/// Parses the `releases/latest` payload. Drafts, pre-releases and tags that
/// are not valid semver yield `None`.
pub fn parse_release(json: &str) -> Result<Option<Release>, UpdateError> {
    parse_release_inner(json, false)
}

/// Like [`parse_release`], but keeps pre-releases.
pub fn parse_release_allowing_prerelease(json: &str) -> Result<Option<Release>, UpdateError> {
    parse_release_inner(json, true)
}

fn parse_release_inner(json: &str, allow_prerelease: bool) -> Result<Option<Release>, UpdateError> {
    let raw: RawRelease = serde_json::from_str(json)
        .map_err(|e| UpdateError::Other(format!("bad release data: {e}")))?;
    if raw.draft || (!allow_prerelease && raw.prerelease) {
        return Ok(None);
    }
    let Some(version) = parse_version(&raw.tag_name) else {
        return Ok(None);
    };
    Ok(Some(Release {
        version,
        notes: raw.body.unwrap_or_default().replace("\r\n", "\n"),
        assets: raw
            .assets
            .into_iter()
            .map(|asset| Asset {
                name: asset.name,
                url: asset.browser_download_url,
            })
            .collect(),
    }))
}

/// The portable archive cube publishes for this platform.
const fn portable_suffix() -> &'static str {
    if cfg!(windows) {
        "-portable-x64.zip"
    } else if cfg!(target_os = "macos") {
        "-macos-arm64.tar.gz"
    } else {
        "-linux-x64.tar.gz"
    }
}

/// The checksum listing that covers this platform's assets.
const fn sums_name() -> &'static str {
    if cfg!(windows) {
        "SHA256SUMS-windows.txt"
    } else if cfg!(target_os = "macos") {
        "SHA256SUMS-macos-arm64.txt"
    } else {
        "SHA256SUMS-linux-x64.txt"
    }
}

impl Release {
    pub fn is_newer_than_current(&self) -> bool {
        is_newer(CURRENT_VERSION, &self.version.to_string())
    }

    /// The asset to download for this kind of install.
    pub fn asset_for(&self, kind: InstallKind) -> Option<&Asset> {
        let suffix = match kind {
            InstallKind::Installed { .. } => "-setup-x64.exe",
            InstallKind::Portable => portable_suffix(),
        };
        self.assets
            .iter()
            .find(|asset| asset.name.starts_with("cube-") && asset.name.ends_with(suffix))
    }

    fn sums_asset(&self) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == sums_name())
    }
}

/// Finds the hash for `file` in a `sha256sum`-style listing
/// (`<hex>  <name>` or `<hex> *<name>`).
pub fn hash_for(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == file && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

fn agent(timeouts: bool) -> ureq::Agent {
    let builder = ureq::Agent::config_builder()
        .https_only(true)
        .user_agent(user_agent())
        .timeout_connect(Some(Duration::from_secs(15)));
    let builder = if timeouts {
        builder.timeout_global(Some(Duration::from_secs(20)))
    } else {
        builder
    };
    builder.build().into()
}

/// Asks GitHub for the newest release on `channel`. `Ok(None)` means there is
/// no usable release (draft / pre-release on the stable channel / odd tag).
pub fn fetch_channel(channel: Channel) -> Result<Option<Release>, UpdateError> {
    match channel {
        Channel::Stable => fetch_latest(),
        Channel::Prerelease => fetch_latest_including_prereleases(),
    }
}

/// Asks GitHub for the latest stable release. `Ok(None)` means there is no
/// usable release (draft / pre-release / odd tag).
pub fn fetch_latest() -> Result<Option<Release>, UpdateError> {
    let response = match agent(true)
        .get(LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(response) => response,
        // The repo has no published release yet.
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    parse_release(&response.into_body().read_to_string()?)
}

/// Newest release including pre-releases (drafts still excluded). Used by
/// the pre-release update channel.
pub fn fetch_latest_including_prereleases() -> Result<Option<Release>, UpdateError> {
    let response = match agent(true)
        .get(RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let releases: Vec<RawRelease> =
        serde_json::from_str(&response.into_body().read_to_string()?)
            .map_err(|error| UpdateError::Other(format!("bad release data: {error}")))?;
    Ok(pick_newest(&releases))
}

/// Newest non-draft release in a `releases` listing, pre-releases included.
fn pick_newest(releases: &[RawRelease]) -> Option<Release> {
    releases
        .iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = parse_version(&release.tag_name)?;
            Some((
                version.clone(),
                Release {
                    version,
                    notes: release
                        .body
                        .clone()
                        .unwrap_or_default()
                        .replace("\r\n", "\n"),
                    assets: release
                        .assets
                        .iter()
                        .map(|asset| Asset {
                            name: asset.name.clone(),
                            url: asset.browser_download_url.clone(),
                        })
                        .collect(),
                },
            ))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, release)| release)
}

// ----------------------------------------------------------------------
// Install kind
// ----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// Installed by the Inno Setup installer.
    Installed { all_users: bool },
    /// Loose executable (unzipped portable build).
    Portable,
}

#[cfg(not(windows))]
pub fn detect_kind() -> InstallKind {
    InstallKind::Portable
}

#[cfg(windows)]
pub fn detect_kind() -> InstallKind {
    let Ok(exe) = std::env::current_exe() else {
        return InstallKind::Portable;
    };
    let Some(dir) = exe.parent() else {
        return InstallKind::Portable;
    };
    // Inno Setup always drops its uninstaller (`unins000.exe`) in the app dir.
    if !dir.join("unins000.exe").is_file() {
        return InstallKind::Portable;
    }
    let all_users = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(std::env::var_os)
        .any(|program_files| dir.starts_with(program_files));
    InstallKind::Installed { all_users }
}

// ----------------------------------------------------------------------
// Cleanup
// ----------------------------------------------------------------------

fn update_dir() -> PathBuf {
    std::env::temp_dir().join(UPDATE_DIR)
}

fn sibling(exe: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = exe.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn remove_leftovers(exe: Option<&Path>) -> bool {
    let mut clean = true;
    let dir = update_dir();
    if dir.exists() && fs::remove_dir_all(&dir).is_err() {
        clean = false;
    }
    if let Some(exe) = exe {
        for suffix in [".old", ".new"] {
            let path = sibling(exe, suffix);
            if path.exists() && fs::remove_file(&path).is_err() {
                clean = false;
            }
        }
    }
    clean
}

/// Removes everything a previous update left behind. Call first thing on
/// startup. The previous process (or the installer) may still be shutting
/// down, so anything that is still locked is retried in the background.
pub fn cleanup() {
    let exe = std::env::current_exe().ok();
    if remove_leftovers(exe.as_deref()) {
        return;
    }
    std::thread::spawn(move || {
        for _ in 0..15 {
            std::thread::sleep(Duration::from_secs(2));
            if remove_leftovers(exe.as_deref()) {
                break;
            }
        }
    });
}

/// Deletes the whole update directory (used after cancel / failure).
pub fn discard_downloads() {
    let _ = fs::remove_dir_all(update_dir());
}

// ----------------------------------------------------------------------
// Download
// ----------------------------------------------------------------------

#[derive(Default)]
pub struct DownloadProgress {
    pub downloaded: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

impl DownloadProgress {
    /// 0..=100, or `None` while the size is unknown.
    pub fn percent(&self) -> Option<f32> {
        let total = self.total.load(Ordering::Relaxed);
        (total > 0).then(|| {
            (self.downloaded.load(Ordering::Relaxed) as f32 / total as f32 * 100.).min(100.)
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// Downloads the asset for `kind` into `%TEMP%\cube-update\`, checking its
/// SHA-256 against the release's checksum listing. Data goes to a `.part`
/// file that is renamed only after the hash matches. On any failure nothing
/// is left on disk.
pub fn download(
    release: &Release,
    kind: InstallKind,
    progress: &DownloadProgress,
) -> Result<PathBuf, UpdateError> {
    let result = download_inner(release, kind, progress);
    if result.is_err() {
        discard_downloads();
    }
    result
}

fn download_inner(
    release: &Release,
    kind: InstallKind,
    progress: &DownloadProgress,
) -> Result<PathBuf, UpdateError> {
    let asset = release.asset_for(kind).ok_or_else(|| {
        UpdateError::Other("this release has no download for your install type".into())
    })?;
    let sums_asset = release
        .sums_asset()
        .ok_or_else(|| UpdateError::Other(format!("this release has no {}", sums_name())))?;

    let http = agent(false);
    let sums = http
        .get(&sums_asset.url)
        .call()?
        .into_body()
        .read_to_string()?;
    let expected = hash_for(&sums, &asset.name)
        .ok_or_else(|| UpdateError::Other(format!("no checksum listed for {}", asset.name)))?;

    let dir = update_dir();
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    let final_path = dir.join(&asset.name);
    let part_path = dir.join(format!("{}.part", asset.name));

    let response = http.get(&asset.url).call()?;
    progress.total.store(
        response.body().content_length().unwrap_or(0),
        Ordering::Relaxed,
    );
    progress.downloaded.store(0, Ordering::Relaxed);
    let mut reader = response
        .into_body()
        .into_with_config()
        .limit(u64::MAX)
        .reader();

    let mut file = File::create(&part_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err(UpdateError::Cancelled);
        }
        let read = reader.read(&mut buffer).map_err(|error| {
            // ureq reports network failures through io::Error.
            UpdateError::Other(format!("download interrupted: {error}"))
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])?;
        progress
            .downloaded
            .fetch_add(read as u64, Ordering::Relaxed);
    }
    file.flush()?;
    drop(file);

    if hex(&hasher.finalize()) != expected {
        return Err(UpdateError::HashMismatch);
    }
    fs::rename(&part_path, &final_path)?;
    Ok(final_path)
}

// ----------------------------------------------------------------------
// Apply
// ----------------------------------------------------------------------

#[cfg(windows)]
fn spawn_detached(cmd: &mut std::process::Command) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
fn spawn_detached(cmd: &mut std::process::Command) -> std::io::Result<()> {
    cmd.spawn().map(|_| ())
}

/// Applies the verified download. On `Ok` the caller must exit right away: a
/// new cube is (or is about to be) running.
pub fn apply(downloaded: &Path, kind: InstallKind) -> Result<(), UpdateError> {
    match kind {
        InstallKind::Installed { all_users } => {
            let scope = if all_users {
                "/ALLUSERS"
            } else {
                "/CURRENTUSER"
            };
            spawn_detached(std::process::Command::new(downloaded).args([
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/CLOSEAPPLICATIONS",
                scope,
                "/RELAUNCH=1",
            ]))?;
            // The installer runs from the update dir, so that dir is cleaned
            // on the next launch (it can't be deleted while setup is running).
            Ok(())
        }
        InstallKind::Portable => {
            apply_portable(downloaded)?;
            discard_downloads();
            Ok(())
        }
    }
}

fn apply_portable(archive: &Path) -> Result<(), UpdateError> {
    let exe = std::env::current_exe()?;
    let new = sibling(&exe, ".new");
    let old = sibling(&exe, ".old");

    extract_exe(archive, &new).inspect_err(|_| {
        let _ = fs::remove_file(&new);
    })?;

    // Keep the executable bit (and any other permissions) on Unix.
    #[cfg(unix)]
    {
        if let Ok(metadata) = fs::metadata(&exe) {
            let _ = fs::set_permissions(&new, metadata.permissions());
        }
    }

    let _ = fs::remove_file(&old);
    // Windows lets us rename a running executable.
    fs::rename(&exe, &old).inspect_err(|_| {
        let _ = fs::remove_file(&new);
    })?;
    if let Err(error) = fs::rename(&new, &exe) {
        let _ = fs::rename(&old, &exe);
        let _ = fs::remove_file(&new);
        return Err(error.into());
    }
    if let Err(error) = spawn_detached(&mut std::process::Command::new(&exe)) {
        // Roll back so the user keeps a working copy.
        let _ = fs::remove_file(&exe);
        let _ = fs::rename(&old, &exe);
        return Err(error.into());
    }
    Ok(())
}

/// Writes the `cube.exe` entry of the portable zip to `dest`.
#[cfg(windows)]
fn extract_exe(zip_path: &Path, dest: &Path) -> Result<(), UpdateError> {
    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)
        .map_err(|error| UpdateError::Other(format!("bad zip: {error}")))?;
    let index = (0..archive.len())
        .find(|&index| {
            archive.by_index_raw(index).is_ok_and(|entry| {
                entry.name().is_ok_and(|name| {
                    Path::new(name.as_ref())
                        .file_name()
                        .is_some_and(|file_name| file_name.eq_ignore_ascii_case("cube.exe"))
                })
            })
        })
        .ok_or_else(|| UpdateError::Other("cube.exe is missing from the zip".into()))?;
    let mut entry = archive
        .by_index(index)
        .map_err(|error| UpdateError::Other(format!("bad zip: {error}")))?;
    let mut out = File::create(dest)?;
    std::io::copy(&mut entry, &mut out)?;
    out.flush()?;
    Ok(())
}

/// Writes the `cube` entry of the portable tarball to `dest`.
#[cfg(unix)]
fn extract_exe(tarball_path: &Path, dest: &Path) -> Result<(), UpdateError> {
    use flate2::read::GzDecoder;

    let file = File::open(tarball_path)?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    let entries = archive
        .entries()
        .map_err(|error| UpdateError::Other(format!("bad tarball: {error}")))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|error| UpdateError::Other(format!("bad tarball: {error}")))?;
        let path = entry
            .path()
            .map_err(|error| UpdateError::Other(format!("bad tarball: {error}")))?;
        if path.file_name().is_some_and(|name| name == "cube") {
            entry
                .unpack(dest)
                .map_err(|error| UpdateError::Other(format!("could not extract cube: {error}")))?;
            return Ok(());
        }
    }
    Err(UpdateError::Other(
        "cube is missing from the tarball".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_json(tag: &str, draft: bool, prerelease: bool) -> String {
        format!(
            r#"{{
                "tag_name": "{tag}", "draft": {draft}, "prerelease": {prerelease},
                "body": "line1\r\nline2",
                "assets": [
                    {{"name": "cube-0.2.0-setup-x64.exe", "browser_download_url": "https://x/setup.exe"}},
                    {{"name": "cube-0.2.0-portable-x64.zip", "browser_download_url": "https://x/p.zip"}},
                    {{"name": "cube-0.2.0-linux-x64.tar.gz", "browser_download_url": "https://x/p.tar.gz"}},
                    {{"name": "cube-0.2.0-macos-arm64.tar.gz", "browser_download_url": "https://x/p-mac.tar.gz"}},
                    {{"name": "SHA256SUMS-windows.txt", "browser_download_url": "https://x/sums-windows"}},
                    {{"name": "SHA256SUMS-linux-x64.txt", "browser_download_url": "https://x/sums-linux"}},
                    {{"name": "SHA256SUMS-macos-arm64.txt", "browser_download_url": "https://x/sums-macos"}}
                ]
            }}"#
        )
    }

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.1.0", "v0.1.1"));
        assert!(is_newer("0.1.9", "0.2.0"));
        assert!(is_newer("0.9.0", "v0.10.0"));
        assert!(!is_newer("0.1.0", "v0.1.0"));
        assert!(!is_newer("0.2.0", "v0.1.9"));
        assert!(!is_newer("0.1.0", "not-a-version"));
        // A stable release is newer than its own pre-release.
        assert!(is_newer("0.2.0-beta.1", "0.2.0"));
    }

    #[test]
    fn parses_release_and_selects_assets() {
        let release = parse_release(&release_json("v0.2.0", false, false))
            .unwrap()
            .unwrap();
        assert_eq!(release.version, Version::new(0, 2, 0));
        assert_eq!(release.notes, "line1\nline2");
        assert_eq!(
            release
                .asset_for(InstallKind::Installed { all_users: false })
                .unwrap()
                .name,
            "cube-0.2.0-setup-x64.exe"
        );
        assert_eq!(
            release
                .asset_for(InstallKind::Installed { all_users: true })
                .unwrap()
                .name,
            "cube-0.2.0-setup-x64.exe"
        );
        assert_eq!(
            release.asset_for(InstallKind::Portable).unwrap().name,
            format!("cube-0.2.0{}", portable_suffix())
        );
        assert_eq!(release.sums_asset().unwrap().name, sums_name());
    }

    #[test]
    fn missing_asset_is_none() {
        let mut release = parse_release(&release_json("v0.2.0", false, false))
            .unwrap()
            .unwrap();
        release
            .assets
            .retain(|asset| !asset.name.ends_with(portable_suffix()));
        assert!(release.asset_for(InstallKind::Portable).is_none());
    }

    #[test]
    fn ignores_drafts_prereleases_and_bad_tags() {
        assert!(
            parse_release(&release_json("v0.2.0", true, false))
                .unwrap()
                .is_none()
        );
        assert!(
            parse_release(&release_json("v0.2.0", false, true))
                .unwrap()
                .is_none()
        );
        assert!(
            parse_release(&release_json("nightly", false, false))
                .unwrap()
                .is_none()
        );
        assert!(parse_release("{").is_err());
    }

    #[test]
    fn prerelease_channel_picks_the_newest_tag() {
        // The stable parser still rejects pre-releases…
        assert!(
            parse_release(&release_json("v0.3.0-beta.1", false, true))
                .unwrap()
                .is_none()
        );
        // …while the opt-in parser keeps them.
        assert!(
            parse_release_allowing_prerelease(&release_json("v0.3.0-beta.1", false, true))
                .unwrap()
                .is_some()
        );
        let list = format!(
            "[{}, {}, {}]",
            release_json("v0.2.0", false, false),
            release_json("v0.3.0-beta.1", false, true),
            release_json("v9.9.9", true, false),
        );
        let releases: Vec<RawRelease> = serde_json::from_str(&list).unwrap();
        let newest = pick_newest(&releases).unwrap();
        // The draft (however high its tag) loses to the beta.
        assert_eq!(newest.version, Version::parse("0.3.0-beta.1").unwrap());
        assert!(pick_newest(&[]).is_none());
    }

    #[test]
    fn parses_sha256sums() {
        let first = "a".repeat(64);
        let second = "B".repeat(64);
        let sums = format!(
            "{first}  cube-0.2.0-setup-x64.exe\r\n{second} *cube-0.2.0-portable-x64.zip\n\ngarbage\n"
        );
        assert_eq!(hash_for(&sums, "cube-0.2.0-setup-x64.exe"), Some(first));
        assert_eq!(
            hash_for(&sums, "cube-0.2.0-portable-x64.zip"),
            Some("b".repeat(64))
        );
        assert_eq!(hash_for(&sums, "other.exe"), None);
        assert_eq!(hash_for("abc  cube.exe", "cube.exe"), None);
    }

    #[test]
    fn hex_encoding() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
    }

    #[test]
    fn sibling_appends_suffix() {
        assert_eq!(
            sibling(Path::new(r"C:\a\cube.exe"), ".old"),
            PathBuf::from(r"C:\a\cube.exe.old")
        );
    }
}
