use anyhow::Result;
use console::style;
use image::GenericImageView;
use std::path::PathBuf;

use crate::processors::{AssetType, get_model_info, get_audio_info, detect_model_format};

pub fn run(input: PathBuf) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("File does not exist: {}", input.display());
    }

    let asset_type = AssetType::from_path(&input);
    let file_size = std::fs::metadata(&input)?.len();

    println!("{} Asset Information", style("📋").blue().bold());
    println!("  File: {}", style(input.display()).cyan());
    println!("  Size: {}", format_size(file_size));
    println!("  Type: {}", style(format!("{:?}", asset_type)).cyan());
    println!();

    match asset_type {
        AssetType::Image => print_image_info(&input)?,
        AssetType::Model => print_model_info(&input)?,
        AssetType::Audio => print_audio_info(&input)?,
        AssetType::Unknown => {
            println!("  {}", style("Unknown or unsupported file type").yellow());
        }
    }

    Ok(())
}

fn print_image_info(input: &PathBuf) -> Result<()> {
    let img = image::open(input)?;
    let (width, height) = img.dimensions();
    let color_type = img.color();

    println!("  {}", style("Image Properties:").bold());
    println!("    Dimensions: {}x{}", width, height);
    println!("    Color type: {:?}", color_type);
    println!("    Pixels: {}", width * height);

    // Estimate uncompressed size
    let bytes_per_pixel = match color_type {
        image::ColorType::L8 => 1,
        image::ColorType::La8 => 2,
        image::ColorType::Rgb8 => 3,
        image::ColorType::Rgba8 => 4,
        image::ColorType::L16 => 2,
        image::ColorType::La16 => 4,
        image::ColorType::Rgb16 => 6,
        image::ColorType::Rgba16 => 8,
        image::ColorType::Rgb32F => 12,
        image::ColorType::Rgba32F => 16,
        _ => 4,
    };
    let uncompressed = width as u64 * height as u64 * bytes_per_pixel;
    println!("    Uncompressed: {}", format_size(uncompressed));

    // Format detection
    let ext = input.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_uppercase())
        .unwrap_or_else(|| "Unknown".to_string());
    println!("    Format: {}", ext);

    // Compression ratio
    let file_size = std::fs::metadata(input)?.len();
    if uncompressed > 0 {
        let ratio = file_size as f64 / uncompressed as f64;
        println!("    Compression: {:.1}%", ratio * 100.0);
    }

    Ok(())
}

fn print_model_info(input: &PathBuf) -> Result<()> {
    let format = detect_model_format(input);

    println!("  {}", style("Model Properties:").bold());
    if let Some(fmt) = format {
        println!("    Format: {}", fmt);
    }

    // Only process glTF/GLB
    let ext = input.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match ext.as_deref() {
        Some("gltf" | "glb") => {
            let info = get_model_info(input)?;
            println!("    Meshes: {}", info.meshes);
            println!("    Vertices: {}", info.total_vertices);
            println!("    Indices: {}", info.total_indices);
            println!("    Triangles: ~{}", info.total_indices / 3);
            println!("    Materials: {}", info.materials);
            println!("    Textures: {}", info.textures);
            println!("    Animations: {}", info.animations);
            println!("    Nodes: {}", info.nodes);
        }
        _ => {
            println!("    {}", style("Detailed info not available for this format").dim());
        }
    }

    Ok(())
}

fn print_audio_info(input: &PathBuf) -> Result<()> {
    let info = get_audio_info(input)?;

    println!("  {}", style("Audio Properties:").bold());
    println!("    Format: {}", info.format);
    println!("    Channels: {}", info.channels);
    println!("    Sample rate: {} Hz", info.sample_rate);
    println!("    Duration: {:.2}s", info.duration_secs);

    // Bitrate estimate
    let file_size = std::fs::metadata(input)?.len();
    if info.duration_secs > 0.0 {
        let bitrate = (file_size as f64 * 8.0) / info.duration_secs / 1000.0;
        println!("    Bitrate: ~{:.0} kbps", bitrate);
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
