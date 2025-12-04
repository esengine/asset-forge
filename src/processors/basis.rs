use anyhow::{Context, Result};
use basis_universal::{
    BasisTextureFormat, ColorSpace, Compressor, CompressorParams,
    Transcoder, TranscoderTextureFormat,
    transcoding::TranscodeParameters,
};
use image::{DynamicImage, GenericImageView};
use std::path::Path;
use std::time::Instant;

use crate::cli::QualityPreset;
use super::ProcessingStats;

/// Basis Universal compression mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasisCompressionMode {
    /// ETC1S - Smaller file size, lower quality
    Etc1s,
    /// UASTC - Larger file size, higher quality
    Uastc,
}

impl Default for BasisCompressionMode {
    fn default() -> Self {
        Self::Uastc
    }
}

/// Configuration for Basis Universal compression
#[derive(Debug, Clone)]
pub struct BasisConfig {
    pub mode: BasisCompressionMode,
    pub quality: QualityPreset,
    pub generate_mipmaps: bool,
    pub max_size: Option<u32>,
}

impl Default for BasisConfig {
    fn default() -> Self {
        Self {
            mode: BasisCompressionMode::Uastc,
            quality: QualityPreset::Balanced,
            generate_mipmaps: true,
            max_size: None,
        }
    }
}

/// Compress an image to Basis Universal format (.basis file)
pub fn compress_to_basis(
    input: &Path,
    output: &Path,
    config: &BasisConfig,
) -> Result<ProcessingStats> {
    let start = Instant::now();
    let original_size = std::fs::metadata(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?
        .len();

    // Load and optionally resize image
    let img = load_and_resize_image(input, config.max_size)?;
    let (width, height) = img.dimensions();
    let rgba_data = img.to_rgba8();

    // Create output directory if needed
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Set up compressor
    let mut compressor = Compressor::new(1); // 1 image
    let mut params = CompressorParams::new();

    // Configure based on mode
    match config.mode {
        BasisCompressionMode::Etc1s => {
            params.set_basis_format(BasisTextureFormat::ETC1S);
            params.set_etc1s_quality_level(quality_to_etc1s_level(config.quality));
        }
        BasisCompressionMode::Uastc => {
            params.set_basis_format(BasisTextureFormat::UASTC4x4);
            params.set_uastc_quality_level(quality_to_uastc_level(config.quality));
            // Enable RDO (Rate Distortion Optimization) for better compression
            params.set_rdo_uastc(Some(1.0));
        }
    }

    params.set_generate_mipmaps(config.generate_mipmaps);
    params.set_color_space(ColorSpace::Srgb);

    // Set source image
    let mut source_image = params.source_image_mut(0);
    source_image.init(rgba_data.as_raw(), width, height, 4);

    // Compress
    // SAFETY: We have properly initialized the params with valid image data
    unsafe {
        compressor.init(&params);
        compressor.process().map_err(|e| anyhow::anyhow!("Basis compression failed: {:?}", e))?;
    }

    // Get compressed data and write to file
    let basis_data = compressor.basis_file();
    std::fs::write(output, basis_data)
        .with_context(|| format!("Failed to write basis file: {}", output.display()))?;

    let output_size = std::fs::metadata(output)?.len();
    let processing_time_ms = start.elapsed().as_millis() as u64;

    Ok(ProcessingStats {
        original_size,
        output_size,
        processing_time_ms,
    })
}

/// Compress an image to KTX2 format with Basis Universal compression
/// Note: Uses .basis format internally, which can be transcoded to any GPU format
pub fn compress_to_ktx2(
    input: &Path,
    output: &Path,
    config: &BasisConfig,
) -> Result<ProcessingStats> {
    // The basis-universal 0.3 crate outputs .basis format
    // which is universally transcodable to GPU formats at load time
    compress_to_basis(input, output, config)
}

/// Transcode a Basis file to a specific GPU format
#[allow(dead_code)]
pub fn transcode_basis(
    input: &Path,
    target_format: TranscoderTextureFormat,
) -> Result<Vec<u8>> {
    let basis_data = std::fs::read(input)
        .with_context(|| format!("Failed to read basis file: {}", input.display()))?;

    let mut transcoder = Transcoder::new();
    transcoder.prepare_transcoding(&basis_data)
        .map_err(|e| anyhow::anyhow!("Failed to prepare transcoding: {:?}", e))?;

    let _image_info = transcoder.image_info(&basis_data, 0)
        .ok_or_else(|| anyhow::anyhow!("Failed to get image info"))?;

    let params = TranscodeParameters {
        image_index: 0,
        level_index: 0,
        decode_flags: None,
        output_row_pitch_in_blocks_or_pixels: None,
        output_rows_in_pixels: None,
    };

    let transcoded = transcoder.transcode_image_level(&basis_data, target_format, params)
        .map_err(|e| anyhow::anyhow!("Failed to transcode: {:?}", e))?;

    Ok(transcoded)
}

fn load_and_resize_image(path: &Path, max_size: Option<u32>) -> Result<DynamicImage> {
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

fn quality_to_etc1s_level(quality: QualityPreset) -> u32 {
    match quality {
        QualityPreset::Fast => 64,
        QualityPreset::Balanced => 128,
        QualityPreset::High => 192,
        QualityPreset::Ultra => 255,
    }
}

fn quality_to_uastc_level(quality: QualityPreset) -> u32 {
    match quality {
        QualityPreset::Fast => 0,
        QualityPreset::Balanced => 1,
        QualityPreset::High => 2,
        QualityPreset::Ultra => 4,
    }
}

/// Get information about supported transcoding formats
#[allow(dead_code)]
pub fn get_supported_formats() -> Vec<(&'static str, TranscoderTextureFormat)> {
    vec![
        ("BC7 (Desktop)", TranscoderTextureFormat::BC7_RGBA),
        ("BC3 (DX10)", TranscoderTextureFormat::BC3_RGBA),
        ("BC1 (DX10, no alpha)", TranscoderTextureFormat::BC1_RGB),
        ("ETC2 (Mobile)", TranscoderTextureFormat::ETC2_RGBA),
        ("ETC1 (Legacy Mobile)", TranscoderTextureFormat::ETC1_RGB),
        ("ASTC 4x4 (Modern Mobile)", TranscoderTextureFormat::ASTC_4x4_RGBA),
        ("PVRTC1 (iOS)", TranscoderTextureFormat::PVRTC1_4_RGBA),
        ("RGBA32 (Uncompressed)", TranscoderTextureFormat::RGBA32),
    ]
}
