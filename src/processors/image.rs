use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageFormat};
use oxipng::{InFile, Options, OutFile};
use std::path::Path;
use std::time::Instant;

use crate::cli::{OutputFormat, QualityPreset};

use super::{compress_to_ktx2, BasisCompressionMode, BasisConfig, ProcessingStats};

/// Image processor configuration
#[derive(Debug, Clone)]
pub struct ImageProcessorConfig {
    pub output_format: Option<OutputFormat>,
    pub quality: QualityPreset,
    pub max_size: Option<u32>,
    pub generate_mipmaps: bool,
}

impl Default for ImageProcessorConfig {
    fn default() -> Self {
        Self {
            output_format: None,
            quality: QualityPreset::Balanced,
            max_size: None,
            generate_mipmaps: false,
        }
    }
}

/// Process an image file
pub fn process_image(
    input: &Path,
    output: &Path,
    config: &ImageProcessorConfig,
) -> Result<ProcessingStats> {
    let start = Instant::now();
    let original_size = std::fs::metadata(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?
        .len();

    // Determine output format
    let output_format = config.output_format.unwrap_or_else(|| {
        output
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| match e.to_lowercase().as_str() {
                "jpg" | "jpeg" => OutputFormat::Jpeg,
                "webp" => OutputFormat::Webp,
                "ktx2" => OutputFormat::Ktx2,
                _ => OutputFormat::Png,
            })
            .unwrap_or(OutputFormat::Png)
    });

    // Create output directory if needed
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match output_format {
        OutputFormat::Png => process_png(input, output, config)?,
        OutputFormat::Jpeg => process_jpeg(input, output, config)?,
        OutputFormat::Webp => process_webp(input, output, config)?,
        OutputFormat::Ktx2 => {
            let basis_config = BasisConfig {
                mode: BasisCompressionMode::Uastc,
                quality: config.quality,
                generate_mipmaps: config.generate_mipmaps,
                max_size: config.max_size,
            };
            return compress_to_ktx2(input, output, &basis_config);
        }
    }

    let output_size = std::fs::metadata(output)
        .with_context(|| format!("Failed to read output file: {}", output.display()))?
        .len();

    let processing_time_ms = start.elapsed().as_millis() as u64;

    Ok(ProcessingStats {
        original_size,
        output_size,
        processing_time_ms,
    })
}

/// Process PNG using oxipng
fn process_png(input: &Path, output: &Path, config: &ImageProcessorConfig) -> Result<()> {
    // Load and resize if needed
    let img = load_and_resize(input, config.max_size)?;

    // Save as PNG first (if resized or input wasn't PNG)
    let temp_path = if config.max_size.is_some() || !is_png(input) {
        let temp = output.with_extension("tmp.png");
        img.save_with_format(&temp, ImageFormat::Png)?;
        Some(temp)
    } else {
        None
    };

    let default_path = input.to_path_buf();
    let input_path = temp_path.as_ref().unwrap_or(&default_path);

    // Configure oxipng based on quality preset
    let options = match config.quality {
        QualityPreset::Fast => Options::from_preset(1),
        QualityPreset::Balanced => Options::from_preset(3),
        QualityPreset::High => Options::from_preset(5),
        QualityPreset::Ultra => Options::from_preset(6),
    };

    // Run oxipng optimization
    oxipng::optimize(
        &InFile::Path(input_path.clone()),
        &OutFile::from_path(output.to_path_buf()),
        &options,
    )
    .with_context(|| format!("Failed to optimize PNG: {}", input.display()))?;

    // Clean up temp file
    if let Some(temp) = temp_path {
        let _ = std::fs::remove_file(temp);
    }

    Ok(())
}

/// Process JPEG
fn process_jpeg(input: &Path, output: &Path, config: &ImageProcessorConfig) -> Result<()> {
    let img = load_and_resize(input, config.max_size)?;

    let quality = match config.quality {
        QualityPreset::Fast => 70,
        QualityPreset::Balanced => 80,
        QualityPreset::High => 90,
        QualityPreset::Ultra => 95,
    };

    // Use image crate for JPEG encoding
    let mut output_file = std::fs::File::create(output)?;
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output_file, quality);
    img.write_with_encoder(encoder)?;

    Ok(())
}

/// Process WebP
fn process_webp(input: &Path, output: &Path, config: &ImageProcessorConfig) -> Result<()> {
    let img = load_and_resize(input, config.max_size)?;

    // image crate supports WebP encoding
    img.save_with_format(output, ImageFormat::WebP)?;

    Ok(())
}

/// Load an image and optionally resize it
fn load_and_resize(path: &Path, max_size: Option<u32>) -> Result<DynamicImage> {
    let img = image::open(path)
        .with_context(|| format!("Failed to open image: {}", path.display()))?;

    if let Some(max) = max_size {
        let (width, height) = img.dimensions();
        if width > max || height > max {
            let ratio = max as f32 / width.max(height) as f32;
            let new_width = (width as f32 * ratio) as u32;
            let new_height = (height as f32 * ratio) as u32;
            return Ok(img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3));
        }
    }

    Ok(img)
}

fn is_png(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase() == "png")
        .unwrap_or(false)
}

/// Get image dimensions without loading the full image
#[allow(deprecated)]
pub fn get_image_dimensions(path: &Path) -> Result<(u32, u32)> {
    let reader = image::io::Reader::open(path)?;
    let dimensions = reader.into_dimensions()?;
    Ok(dimensions)
}
