//! Regenerate PNG and Windows ICO from the editable SVG:
//! cargo run --locked --example generate_brand_icons
use std::{error::Error, fs, path::Path};

fn render(svg: &[u8], size: u32) -> Result<Vec<u8>, Box<dyn Error>> {
    let image = egui_extras::image::load_svg_bytes_with_size(
        svg,
        Some(eframe::egui::load::SizeHint::Size(size, size)),
    )?;
    Ok(image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect())
}

fn bitmap(size: u32, rgba: &[u8]) -> Vec<u8> {
    let mask_stride = size.div_ceil(32) * 4;
    let mut bytes = Vec::new();
    bytes.extend(40u32.to_le_bytes());
    bytes.extend(size.to_le_bytes());
    bytes.extend((size * 2).to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(32u16.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    bytes.extend((size * size * 4).to_le_bytes());
    bytes.extend([0u8; 16]);
    for y in (0..size as usize).rev() {
        for x in 0..size as usize {
            let at = (y * size as usize + x) * 4;
            bytes.extend([rgba[at + 2], rgba[at + 1], rgba[at], rgba[at + 3]]);
        }
    }
    for y in (0..size as usize).rev() {
        let mut row = vec![0u8; mask_stride as usize];
        for x in 0..size as usize {
            if rgba[(y * size as usize + x) * 4 + 3] == 0 {
                row[x / 8] |= 0x80 >> (x % 8);
            }
        }
        bytes.extend(row);
    }
    bytes
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let svg = fs::read(root.join("assets/islo-logo.svg"))?;
    image::save_buffer_with_format(
        root.join("assets/islo-logo.png"),
        &render(&svg, 1024)?,
        1024,
        1024,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )?;

    let sizes = [16u32, 20, 24, 32, 40, 48, 64, 128, 256];
    let frames: Vec<_> = sizes
        .iter()
        .map(|&size| render(&svg, size).map(|rgba| bitmap(size, &rgba)))
        .collect::<Result<_, _>>()?;
    let mut ico = Vec::new();
    ico.extend(0u16.to_le_bytes());
    ico.extend(1u16.to_le_bytes());
    ico.extend((sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + sizes.len() as u32 * 16;
    for (&size, frame) in sizes.iter().zip(&frames) {
        ico.extend([size as u8, size as u8, 0, 0]);
        ico.extend(1u16.to_le_bytes());
        ico.extend(32u16.to_le_bytes());
        ico.extend((frame.len() as u32).to_le_bytes());
        ico.extend(offset.to_le_bytes());
        offset += frame.len() as u32;
    }
    for frame in frames {
        ico.extend(frame);
    }
    fs::write(root.join("assets/islo.ico"), ico)?;
    println!("Generated islo PNG and ICO (16–256 px) from assets/islo-logo.svg");
    Ok(())
}
