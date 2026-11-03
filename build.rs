//! Rasterizes `assets/logo.svg` into a multi-size `.ico` and embeds it as icon
//! resource #1, which Explorer shows for the exe and GPUI loads for the window.

use std::path::PathBuf;

const ICON_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed=assets/logo.svg");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let ico = render_ico();
    let ico_path = out_dir.join("cube.ico");
    std::fs::write(&ico_path, &ico).expect("write the icon");
    // The installer uses the same icon, and looks for it here.
    std::fs::create_dir_all("target").expect("create target");
    std::fs::write(PathBuf::from("target").join("cube.ico"), &ico).expect("copy the icon");

    let rc_path = out_dir.join("cube.rc");
    std::fs::write(&rc_path, resource_script(&ico_path)).expect("write the resource script");
    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_required()
        .expect("embed the icon resource");
}

/// Builds an ICO container with PNG-compressed entries.
fn render_ico() -> Vec<u8> {
    let svg = std::fs::read("assets/logo.svg").expect("read assets/logo.svg");
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())
        .expect("parse logo.svg");
    let pngs: Vec<Vec<u8>> = ICON_SIZES
        .iter()
        .map(|&size| {
            let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("valid icon size");
            let scale = size as f32 / tree.size().width();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            pixmap.encode_png().expect("encode the icon")
        })
        .collect();

    let mut ico = Vec::new();
    ico.extend_from_slice(&0_u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1_u16.to_le_bytes()); // type: icon
    ico.extend_from_slice(&(ICON_SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * ICON_SIZES.len() as u32;
    for (&size, png) in ICON_SIZES.iter().zip(&pngs) {
        let dimension = if size >= 256 { 0 } else { size as u8 };
        ico.extend_from_slice(&[dimension, dimension, 0, 0]);
        ico.extend_from_slice(&1_u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32_u16.to_le_bytes()); // bits per pixel
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in &pngs {
        ico.extend_from_slice(png);
    }
    ico
}

fn resource_script(ico_path: &std::path::Path) -> String {
    let version = std::env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let mut parts: Vec<u32> = version
        .split(['.', '-'])
        .filter_map(|part| part.parse().ok())
        .collect();
    parts.resize(4, 0);
    let numeric = format!("{},{},{},{}", parts[0], parts[1], parts[2], parts[3]);
    format!(
        r#"1 ICON "{icon}"

1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "FileDescription", "cube - speedcubing timer"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "cube"
      VALUE "OriginalFilename", "cube.exe"
      VALUE "ProductName", "cube"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = ico_path.display().to_string().replace('\\', "/"),
    )
}
