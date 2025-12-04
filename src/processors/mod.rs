mod image;
mod atlas;
mod basis;
mod audio;
mod model;
mod cache;

pub use self::image::*;
pub use atlas::*;
pub use basis::*;
pub use audio::*;
pub use model::*;
pub use cache::*;

use anyhow::Result;
use std::path::Path;

/// Statistics from processing an asset
#[derive(Debug, Clone)]
pub struct ProcessingStats {
    pub original_size: u64,
    pub output_size: u64,
    pub processing_time_ms: u64,
}

impl ProcessingStats {
    pub fn compression_ratio(&self) -> f64 {
        if self.original_size == 0 {
            return 0.0;
        }
        1.0 - (self.output_size as f64 / self.original_size as f64)
    }

    pub fn size_reduction_percent(&self) -> f64 {
        self.compression_ratio() * 100.0
    }
}

/// Detect asset type from file extension
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    Image,
    Model,
    Audio,
    Unknown,
}

impl AssetType {
    pub fn from_path(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match extension.as_deref() {
            // Images (including compressed texture formats)
            Some("png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "tga" | "ktx2" | "basis") => {
                AssetType::Image
            }
            // 3D Models
            Some("gltf" | "glb" | "obj" | "fbx") => AssetType::Model,
            // Audio
            Some("wav" | "mp3" | "ogg" | "flac" | "aac" | "m4a") => AssetType::Audio,
            // Unknown
            _ => AssetType::Unknown,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AssetType::Image => "Image/Texture",
            AssetType::Model => "3D Model",
            AssetType::Audio => "Audio",
            AssetType::Unknown => "Unknown",
        }
    }
}

/// Common trait for all asset processors
pub trait AssetProcessor {
    fn process(&self, input: &Path, output: &Path) -> Result<ProcessingStats>;
    fn supported_extensions(&self) -> &[&str];
}
