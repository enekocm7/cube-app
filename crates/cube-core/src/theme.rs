//! cube-tui theme files.
//!
//! cube-tui stores its themes as TOML files in the `themes` folder of the shared
//! data directory. Both apps read the same files, so one theme works in both.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A color from a theme file, stored as `#RRGGBB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeColor {
    r: u8,
    g: u8,
    b: u8,
}

impl ThemeColor {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };
    pub const BLUE: Self = Self {
        r: 51,
        g: 153,
        b: 255,
    };
    pub const GOLD: Self = Self {
        r: 255,
        g: 215,
        b: 0,
    };

    /// Builds an opaque color from its red, green and blue components.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Returns the color as `0xRRGGBB`.
    pub const fn rgb(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }

    /// Parses a six-digit `#RRGGBB` color.
    pub fn from_hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.is_ascii() {
            return None;
        }
        Some(Self {
            r: u8::from_str_radix(&digits[0..2], 16).ok()?,
            g: u8::from_str_radix(&digits[2..4], 16).ok()?,
            b: u8::from_str_radix(&digits[4..6], 16).ok()?,
        })
    }

    /// Formats the color as an uppercase `#RRGGBB` value.
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Returns whether the color is dark enough for light text.
    pub const fn is_dark(self) -> bool {
        let brightness = (self.r as u32 * 299 + self.g as u32 * 587 + self.b as u32 * 114) / 1000;
        brightness < 128
    }
}

impl Serialize for ThemeColor {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_hex().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        Self::from_hex(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("Invalid hex color: {text}")))
    }
}

/// The colors of a cube-tui theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeColors {
    pub background: ThemeColor,
    pub border: ThemeColor,
    pub scramble: ThemeColor,
    pub selection: ThemeColor,
    pub selection_text: ThemeColor,
    pub text: ThemeColor,
    /// Added by newer themes; older files without it keep the default gold.
    #[serde(default = "default_accent")]
    pub accent: ThemeColor,
}

const fn default_accent() -> ThemeColor {
    ThemeColor::GOLD
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            background: ThemeColor::BLACK,
            border: ThemeColor::WHITE,
            scramble: ThemeColor::WHITE,
            selection: ThemeColor::BLUE,
            selection_text: ThemeColor::BLACK,
            text: ThemeColor::WHITE,
            accent: ThemeColor::GOLD,
        }
    }
}

/// A theme file that was read successfully.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeFile {
    /// The file name, including its `.toml` extension.
    pub name: String,
    pub colors: ThemeColors,
}

/// Loads a theme from an exact path.
pub fn load(path: &Path) -> Result<ThemeColors> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("Could not read theme {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("Invalid theme in {}", path.display()))
}

/// Loads a theme by file name, adding `.toml` when it is missing.
pub fn load_named(dir: &Path, name: &str) -> Result<ThemeColors> {
    load(&path_for(dir, name))
}

/// Returns every valid theme in `dir`, sorted by file name.
///
/// Files that cannot be read or parsed are skipped, so one broken theme cannot
/// hide the others.
pub fn list(dir: &Path) -> Result<Vec<ThemeFile>> {
    let entries =
        fs::read_dir(dir).with_context(|| format!("Could not list themes in {}", dir.display()))?;
    let mut themes = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !has_toml_extension(&path) {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let Some(colors) = load(&path).ok() else {
            continue;
        };
        themes.push(ThemeFile {
            name: name.to_owned(),
            colors,
        });
    }
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(themes)
}

/// Writes the default theme to `path` unless a file already exists there.
pub fn write_default(path: &Path) -> Result<()> {
    let text = toml::to_string_pretty(&ThemeColors::default())
        .context("Could not serialize the default theme")?;
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Could not create theme {}", path.display()));
        }
    };
    file.write_all(text.as_bytes())
        .with_context(|| format!("Could not create theme {}", path.display()))
}

fn path_for(dir: &Path, name: &str) -> PathBuf {
    let mut file_name = name.trim().to_owned();
    if !has_toml_extension(Path::new(&file_name)) {
        file_name.push_str(".toml");
    }
    dir.join(file_name)
}

/// Why a theme name cannot become a file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeNameError {
    Empty,
    Invalid,
    TooLong,
}

/// Why a theme could not be saved.
#[derive(Debug)]
pub enum ThemeSaveError {
    Name(ThemeNameError),
    /// A different theme already uses that file name.
    AlreadyExists,
    Failed(anyhow::Error),
}

/// The longest theme name accepted, in bytes.
const MAX_THEME_NAME: usize = 80;

/// Turns a display name into the `.toml` file name used in the themes folder.
pub fn file_name(name: &str) -> std::result::Result<String, ThemeNameError> {
    let stem = strip_toml_suffix(name.trim());
    if stem.is_empty() {
        return Err(ThemeNameError::Empty);
    }
    if stem.len() > MAX_THEME_NAME {
        return Err(ThemeNameError::TooLong);
    }
    if stem.starts_with('.')
        || stem == "."
        || stem == ".."
        || stem.ends_with(' ')
        || stem.ends_with('.')
        || stem.chars().any(|c| {
            c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
        || is_reserved_windows_name(stem)
    {
        return Err(ThemeNameError::Invalid);
    }
    Ok(format!("{stem}.toml"))
}

/// Strips the `.toml` extension from a theme file name for display.
pub fn display_name(file_name: &str) -> &str {
    strip_toml_suffix(file_name)
}

/// Saves `colors` as `name`, removing `previous` when the theme was renamed.
///
/// Existing themes are only overwritten when they are the theme being edited;
/// a new name that is already taken returns [`ThemeSaveError::AlreadyExists`].
pub fn save(
    dir: &Path,
    name: &str,
    colors: &ThemeColors,
    previous: Option<&str>,
) -> std::result::Result<String, ThemeSaveError> {
    let file_name = file_name(name).map_err(ThemeSaveError::Name)?;
    let path = dir.join(&file_name);
    let same_file = previous.is_some_and(|previous| previous.eq_ignore_ascii_case(&file_name));
    let exists = path.try_exists().map_err(|error| {
        ThemeSaveError::Failed(
            anyhow::Error::new(error)
                .context(format!("Could not check whether {} exists", path.display())),
        )
    })?;
    if exists && !same_file {
        return Err(ThemeSaveError::AlreadyExists);
    }
    let text = toml::to_string_pretty(colors).map_err(|error| {
        ThemeSaveError::Failed(anyhow::Error::new(error).context("Could not serialize the theme"))
    })?;
    crate::persistence::write_atomic(&path, text.as_bytes()).map_err(|error| {
        ThemeSaveError::Failed(
            anyhow::Error::new(error).context(format!("Could not save theme {}", path.display())),
        )
    })?;

    if let Some(previous) = previous
        && !previous.eq_ignore_ascii_case(&file_name)
    {
        let previous_path = dir.join(previous);
        if previous_path.try_exists().map_err(|error| {
            ThemeSaveError::Failed(anyhow::Error::new(error).context(format!(
                "Could not check the old theme {}",
                previous_path.display()
            )))
        })? {
            fs::remove_file(&previous_path).map_err(|error| {
                ThemeSaveError::Failed(anyhow::Error::new(error).context(format!(
                    "Could not remove the old theme {}",
                    previous_path.display()
                )))
            })?;
        }
    }
    Ok(file_name)
}

/// Deletes a theme file from `dir`.
pub fn delete(dir: &Path, file_name: &str) -> Result<()> {
    let path = dir.join(file_name);
    fs::remove_file(&path).with_context(|| format!("Could not delete theme {}", path.display()))
}

fn strip_toml_suffix(name: &str) -> &str {
    let length = name.len();
    if length >= 5
        && name.is_char_boundary(length - 5)
        && name[length - 5..].eq_ignore_ascii_case(".toml")
    {
        &name[..length - 5]
    } else {
        name
    }
}

fn is_reserved_windows_name(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    let base = upper.split('.').next().unwrap_or_default();
    matches!(
        base,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// A way of writing a color in the theme editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorFormat {
    #[default]
    Hex,
    Rgb,
    Rgba,
    Hsl,
    Hsla,
}

impl ColorFormat {
    /// Every format, in the order shown in the editor.
    pub const ALL: [Self; 5] = [Self::Hex, Self::Rgb, Self::Rgba, Self::Hsl, Self::Hsla];

    /// Returns the short name shown on the format picker.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::Rgb => "RGB",
            Self::Rgba => "RGBA",
            Self::Hsl => "HSL",
            Self::Hsla => "HSLA",
        }
    }

    /// Parses a color in this format, accepting the usual CSS spellings.
    pub fn parse(self, text: &str) -> Option<ThemeColor> {
        match self {
            Self::Hex => parse_hex(text),
            Self::Rgb | Self::Rgba => parse_rgb(text, self),
            Self::Hsl | Self::Hsla => parse_hsl(text, self),
        }
    }

    /// Formats a color in this format. Theme colors are opaque, so alpha is
    /// always written as `1`.
    pub fn format(self, color: ThemeColor) -> String {
        let (red, green, blue) = (color.r, color.g, color.b);
        match self {
            Self::Hex => color.to_hex(),
            Self::Rgb => format!("rgb({red}, {green}, {blue})"),
            Self::Rgba => format!("rgba({red}, {green}, {blue}, 1)"),
            Self::Hsl => {
                let (hue, saturation, lightness) = rgb_to_hsl(color);
                format!("hsl({hue}, {saturation}%, {lightness}%)")
            }
            Self::Hsla => {
                let (hue, saturation, lightness) = rgb_to_hsl(color);
                format!("hsla({hue}, {saturation}%, {lightness}%, 1)")
            }
        }
    }
}

fn parse_hex(text: &str) -> Option<ThemeColor> {
    let digits = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if !digits.is_ascii() {
        return None;
    }
    let pair = |index: usize| u8::from_str_radix(&digits[index..index + 2], 16).ok();
    let nibble = |index: usize| {
        u8::from_str_radix(&digits[index..=index], 16)
            .ok()
            .map(|value| value * 17)
    };
    match digits.len() {
        3 | 4 => Some(ThemeColor::new(nibble(0)?, nibble(1)?, nibble(2)?)),
        6 | 8 => Some(ThemeColor::new(pair(0)?, pair(2)?, pair(4)?)),
        _ => None,
    }
}

fn parse_rgb(text: &str, format: ColorFormat) -> Option<ThemeColor> {
    let name = if format == ColorFormat::Rgba {
        "rgba"
    } else {
        "rgb"
    };
    let parts = components(text, name)?;
    if parts.len() < 3 {
        return None;
    }
    if parts.len() > 3 && parse_alpha(parts[3]).is_none() {
        return None;
    }
    Some(ThemeColor::new(
        rgb_channel(parts[0])?,
        rgb_channel(parts[1])?,
        rgb_channel(parts[2])?,
    ))
}

fn parse_hsl(text: &str, format: ColorFormat) -> Option<ThemeColor> {
    let name = if format == ColorFormat::Hsla {
        "hsla"
    } else {
        "hsl"
    };
    let parts = components(text, name)?;
    if parts.len() < 3 {
        return None;
    }
    if parts.len() > 3 && parse_alpha(parts[3]).is_none() {
        return None;
    }
    let hue = parts[0]
        .trim()
        .trim_end_matches("deg")
        .parse::<f32>()
        .ok()?
        .rem_euclid(360.0)
        / 360.0;
    let saturation = unit_interval(parts[1])?;
    let lightness = unit_interval(parts[2])?;
    Some(hsl_to_rgb(hue, saturation, lightness))
}

fn components<'a>(text: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let text = text.trim();
    let body = if text
        .get(..name.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
    {
        let rest = text[name.len()..].trim_start();
        rest.strip_prefix('(')?.strip_suffix(')')?
    } else {
        text
    };
    let parts: Vec<&str> = body
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    (!parts.is_empty()).then_some(parts)
}

fn rgb_channel(part: &str) -> Option<u8> {
    if let Some(percent) = part.strip_suffix('%') {
        let percent = percent.trim().parse::<f32>().ok()?;
        if !(0.0..=100.0).contains(&percent) {
            return None;
        }
        Some((percent * 2.55).round() as u8)
    } else {
        part.trim().parse().ok()
    }
}

fn unit_interval(part: &str) -> Option<f32> {
    let (number, percent) = part
        .strip_suffix('%')
        .map_or((part, false), |number| (number, true));
    let number = number.trim().parse::<f32>().ok()?;
    let fraction = if percent || number > 1.0 {
        number / 100.0
    } else {
        number
    };
    (0.0..=1.0).contains(&fraction).then_some(fraction)
}

fn parse_alpha(part: &str) -> Option<f32> {
    unit_interval(part)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> ThemeColor {
    if saturation <= 0.0 {
        let value = channel(lightness);
        return ThemeColor::new(value, value, value);
    }
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue * 6.0;
    let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let offset = lightness - chroma / 2.0;
    ThemeColor::new(
        channel(r + offset),
        channel(g + offset),
        channel(b + offset),
    )
}

fn rgb_to_hsl(color: ThemeColor) -> (u16, u16, u16) {
    let red = f32::from(color.r) / 255.0;
    let green = f32::from(color.g) / 255.0;
    let blue = f32::from(color.b) / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = f32::midpoint(max, min);
    let delta = max - min;
    if delta <= f32::EPSILON {
        return (0, 0, (lightness * 100.0).round() as u16);
    }
    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue = if (max - red).abs() <= f32::EPSILON {
        ((green - blue) / delta).rem_euclid(6.0)
    } else if (max - green).abs() <= f32::EPSILON {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    };
    (
        ((hue / 6.0 * 360.0).round() as u16) % 360,
        (saturation * 100.0).round() as u16,
        (lightness * 100.0).round() as u16,
    )
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn has_toml_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    const CUBE_TUI_THEME: &str = r##"
        background = "#000000"
        border = "#FFFFFF"
        scramble = "#FFFFFF"
        selection = "#3399FF"
        selection_text = "#000000"
        text = "#FFFFFF"
    "##;

    fn temporary_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cube-theme-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_the_documented_cube_tui_theme() {
        let colors: ThemeColors = toml::from_str(CUBE_TUI_THEME).unwrap();
        assert_eq!(colors.background, ThemeColor::BLACK);
        assert_eq!(colors.border, ThemeColor::WHITE);
        assert_eq!(colors.scramble, ThemeColor::WHITE);
        assert_eq!(colors.selection, ThemeColor::BLUE);
        assert_eq!(colors.selection_text, ThemeColor::BLACK);
        assert_eq!(colors.text, ThemeColor::WHITE);
    }

    #[test]
    fn older_themes_without_an_accent_default_to_gold() {
        let colors: ThemeColors = toml::from_str(CUBE_TUI_THEME).unwrap();
        assert_eq!(colors.accent, ThemeColor::GOLD);
    }

    #[test]
    fn a_missing_required_color_is_an_error() {
        let theme = CUBE_TUI_THEME.replace("background = \"#000000\"", "");
        assert!(toml::from_str::<ThemeColors>(&theme).is_err());
    }

    #[test]
    fn colors_round_trip_as_uppercase_hex() {
        let colors: ThemeColors =
            toml::from_str(&format!("{CUBE_TUI_THEME}\naccent = '#00ff00'")).unwrap();
        let text = toml::to_string(&colors).unwrap();
        assert!(text.contains("accent = \"#00FF00\""));
        assert_eq!(toml::from_str::<ThemeColors>(&text).unwrap(), colors);
    }

    #[test]
    fn invalid_colors_are_rejected_without_panicking() {
        assert!(ThemeColor::from_hex("#12345").is_none());
        assert!(ThemeColor::from_hex("123456").is_none());
        assert!(ThemeColor::from_hex("#a\u{0}abc").is_none());
        assert!(toml::from_str::<ThemeColors>("background = '#12345'").is_err());
    }

    #[test]
    fn names_with_or_without_an_extension_load_the_same_theme() {
        let dir = temporary_dir();
        fs::write(dir.join("mine.toml"), CUBE_TUI_THEME).unwrap();
        assert_eq!(
            load_named(&dir, "mine").unwrap(),
            load_named(&dir, "mine.toml").unwrap()
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn listings_skip_broken_files_and_sort_by_name() {
        let dir = temporary_dir();
        fs::write(dir.join("z.toml"), CUBE_TUI_THEME).unwrap();
        fs::write(dir.join("a.TOML"), CUBE_TUI_THEME).unwrap();
        fs::write(dir.join("broken.toml"), "background = 'nope'").unwrap();
        fs::write(dir.join("notes.txt"), CUBE_TUI_THEME).unwrap();
        let themes = list(&dir).unwrap();
        let names: Vec<&str> = themes.iter().map(|theme| theme.name.as_str()).collect();
        assert_eq!(names, ["a.TOML", "z.toml"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_default_theme_is_created_only_when_missing() {
        let dir = temporary_dir();
        let path = dir.join("default.toml");
        write_default(&path).unwrap();
        assert_eq!(load(&path).unwrap(), ThemeColors::default());
        fs::write(&path, CUBE_TUI_THEME).unwrap();
        write_default(&path).unwrap();
        assert_eq!(
            load(&path).unwrap().background,
            ThemeColor::BLACK,
            "an existing theme must never be overwritten"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn brightness_decides_the_palette_mode() {
        assert!(ThemeColor::BLACK.is_dark());
        assert!(!ThemeColor::WHITE.is_dark());
        assert!(ThemeColor::from_hex("#0A0A0A").unwrap().is_dark());
    }

    #[test]
    fn every_format_parses_its_own_output() {
        let red = ThemeColor::new(255, 0, 0);
        for format in ColorFormat::ALL {
            let text = format.format(red);
            assert_eq!(
                format.parse(&text),
                Some(red),
                "{format:?} did not round-trip {text}"
            );
        }
    }

    #[test]
    fn color_inputs_accept_css_spellings() {
        assert_eq!(
            ColorFormat::Hex.parse("#F00"),
            Some(ThemeColor::new(255, 0, 0))
        );
        assert_eq!(
            ColorFormat::Hex.parse("FF000080"),
            Some(ThemeColor::new(255, 0, 0))
        );
        assert_eq!(
            ColorFormat::Rgb.parse("rgb(255, 0, 0)"),
            Some(ThemeColor::new(255, 0, 0))
        );
        assert_eq!(
            ColorFormat::Rgb.parse("100% 0% 0%"),
            Some(ThemeColor::new(255, 0, 0))
        );
        assert_eq!(
            ColorFormat::Rgba.parse("rgba(0, 128, 255, 0.5)"),
            Some(ThemeColor::new(0, 128, 255))
        );
        assert_eq!(
            ColorFormat::Hsl.parse("hsl(120, 100%, 50%)"),
            Some(ThemeColor::new(0, 255, 0))
        );
        assert_eq!(
            ColorFormat::Hsla.parse("hsla(240 100% 50% / 0.2)"),
            Some(ThemeColor::new(0, 0, 255))
        );
        assert_eq!(ColorFormat::Rgb.parse("rgb(300, 0, 0)"), None);
        assert_eq!(ColorFormat::Hsl.parse("hsl(0, 50%, 150%)"), None);
        assert_eq!(ColorFormat::Rgba.parse("rgba(0, 0, 0, nope)"), None);
    }

    #[test]
    fn theme_names_become_safe_file_names() {
        assert_eq!(file_name("My theme").unwrap(), "My theme.toml");
        assert_eq!(file_name("  My theme.toml ").unwrap(), "My theme.toml");
        assert_eq!(file_name("My theme.TOML").unwrap(), "My theme.toml");
        assert_eq!(file_name(""), Err(ThemeNameError::Empty));
        assert_eq!(file_name(".."), Err(ThemeNameError::Invalid));
        assert_eq!(file_name("a/b"), Err(ThemeNameError::Invalid));
        assert_eq!(file_name("bad:name"), Err(ThemeNameError::Invalid));
        assert_eq!(file_name("CON"), Err(ThemeNameError::Invalid));
        assert_eq!(
            file_name(&"x".repeat(MAX_THEME_NAME + 1)),
            Err(ThemeNameError::TooLong)
        );
    }

    #[test]
    fn display_names_drop_the_extension() {
        assert_eq!(display_name("My theme.toml"), "My theme");
        assert_eq!(display_name("My theme.TOML"), "My theme");
        assert_eq!(display_name("My theme"), "My theme");
    }

    #[test]
    fn saves_edits_and_renames_theme_files() {
        let dir = temporary_dir();
        let colors = ThemeColors::default();
        assert_eq!(save(&dir, "Mine", &colors, None).unwrap(), "Mine.toml");
        assert_eq!(load_named(&dir, "Mine").unwrap(), colors);

        let changed = ThemeColors {
            background: ThemeColor::WHITE,
            ..colors
        };
        assert_eq!(
            save(&dir, "Mine", &changed, Some("Mine.toml")).unwrap(),
            "Mine.toml"
        );
        assert_eq!(
            load_named(&dir, "Mine").unwrap().background,
            ThemeColor::WHITE
        );

        assert_eq!(
            save(&dir, "Renamed", &colors, Some("Mine.toml")).unwrap(),
            "Renamed.toml"
        );
        assert_eq!(load_named(&dir, "Renamed").unwrap(), colors);
        assert!(!dir.join("Mine.toml").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saving_never_overwrites_another_theme() {
        let dir = temporary_dir();
        let colors = ThemeColors::default();
        save(&dir, "Taken", &colors, None).unwrap();
        assert!(matches!(
            save(&dir, "Taken", &colors, Some("Other.toml")),
            Err(ThemeSaveError::AlreadyExists)
        ));
        assert!(matches!(
            save(&dir, "Taken", &colors, None),
            Err(ThemeSaveError::AlreadyExists)
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn deletes_theme_files() {
        let dir = temporary_dir();
        save(&dir, "Gone", &ThemeColors::default(), None).unwrap();
        delete(&dir, "Gone.toml").unwrap();
        assert!(!dir.join("Gone.toml").exists());
        assert!(delete(&dir, "Gone.toml").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
