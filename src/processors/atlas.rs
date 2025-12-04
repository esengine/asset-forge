use anyhow::{Context, Result};
use image::{GenericImageView, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use texture_packer::{TexturePacker, TexturePackerConfig};
use texture_packer::exporter::ImageExporter;
use texture_packer::importer::ImageImporter;

use super::ProcessingStats;

/// Configuration for atlas generation
#[derive(Debug, Clone)]
pub struct AtlasConfig {
    pub max_width: u32,
    pub max_height: u32,
    pub padding: u32,
    pub trim: bool,
    pub allow_rotation: bool,
}

impl Default for AtlasConfig {
    fn default() -> Self {
        Self {
            max_width: 2048,
            max_height: 2048,
            padding: 2,
            trim: false,
            allow_rotation: false,
        }
    }
}

/// Metadata for a sprite in the atlas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpriteFrame {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rotated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_x: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim_y: Option<u32>,
}

/// Atlas metadata (JSON output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasMetadata {
    pub image: String,
    pub width: u32,
    pub height: u32,
    pub frames: HashMap<String, SpriteFrame>,
}

/// Result of atlas generation
pub struct AtlasResult {
    pub image: RgbaImage,
    pub metadata: AtlasMetadata,
    pub stats: ProcessingStats,
}

/// Generate a sprite atlas from a directory of images
pub fn generate_atlas(
    input_dir: &Path,
    output_image: &Path,
    config: &AtlasConfig,
) -> Result<AtlasResult> {
    let start = Instant::now();
    let mut total_input_size: u64 = 0;

    // Configure texture packer
    let packer_config = TexturePackerConfig {
        max_width: config.max_width,
        max_height: config.max_height,
        allow_rotation: config.allow_rotation,
        border_padding: config.padding,
        texture_padding: config.padding,
        trim: config.trim,
        ..Default::default()
    };

    let mut packer = TexturePacker::new_skyline(packer_config);

    // Find all image files in the directory
    let image_extensions = ["png", "jpg", "jpeg", "bmp", "gif", "tga"];
    let mut image_paths: Vec<_> = std::fs::read_dir(input_dir)
        .with_context(|| format!("Failed to read directory: {}", input_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .map(|e| image_extensions.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();

    // Sort for deterministic output
    image_paths.sort();

    if image_paths.is_empty() {
        anyhow::bail!("No image files found in directory: {}", input_dir.display());
    }

    // Pack each image
    for path in &image_paths {
        let metadata = std::fs::metadata(path)?;
        total_input_size += metadata.len();

        let texture = ImageImporter::import_from_file(path)
            .map_err(|e| anyhow::anyhow!("Failed to import image '{}': {}", path.display(), e))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        packer.pack_own(name, texture).map_err(|e| {
            anyhow::anyhow!(
                "Failed to pack '{}': {:?}. Try increasing atlas size or reducing sprite count.",
                path.display(),
                e
            )
        })?;
    }

    // Export the atlas image
    let exporter = ImageExporter::export(&packer, None)
        .map_err(|e| anyhow::anyhow!("Failed to export atlas image: {}", e))?;

    // Create output directory if needed
    if let Some(parent) = output_image.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Save the atlas image
    exporter.save(output_image)
        .with_context(|| format!("Failed to save atlas image: {}", output_image.display()))?;

    // Build metadata
    let mut frames = HashMap::new();
    for (name, frame) in packer.get_frames() {
        frames.insert(
            name.clone(),
            SpriteFrame {
                x: frame.frame.x,
                y: frame.frame.y,
                width: frame.frame.w,
                height: frame.frame.h,
                rotated: frame.rotated,
                source_width: if frame.trimmed {
                    Some(frame.source.w)
                } else {
                    None
                },
                source_height: if frame.trimmed {
                    Some(frame.source.h)
                } else {
                    None
                },
                trim_x: if frame.trimmed {
                    Some(frame.source.x)
                } else {
                    None
                },
                trim_y: if frame.trimmed {
                    Some(frame.source.y)
                } else {
                    None
                },
            },
        );
    }

    let output_size = std::fs::metadata(output_image)?.len();
    let processing_time_ms = start.elapsed().as_millis() as u64;

    let metadata = AtlasMetadata {
        image: output_image
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("atlas.png")
            .to_string(),
        width: exporter.width(),
        height: exporter.height(),
        frames,
    };

    // Load the saved image for return
    let atlas_image = image::open(output_image)?.to_rgba8();

    Ok(AtlasResult {
        image: atlas_image,
        metadata,
        stats: ProcessingStats {
            original_size: total_input_size,
            output_size,
            processing_time_ms,
        },
    })
}

/// Save atlas metadata to JSON file
pub fn save_atlas_metadata(metadata: &AtlasMetadata, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(metadata)?;
    std::fs::write(path, json)
        .with_context(|| format!("Failed to write metadata: {}", path.display()))?;
    Ok(())
}
