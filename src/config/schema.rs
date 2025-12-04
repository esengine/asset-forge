use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Root configuration structure for asset-forge.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Project metadata
    #[serde(default)]
    pub project: ProjectConfig,

    /// Platform presets
    #[serde(default)]
    pub presets: HashMap<String, PresetConfig>,

    /// File pattern rules
    #[serde(default)]
    pub rules: HashMap<String, RuleConfig>,

    /// Cache configuration
    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name
    #[serde(default = "default_project_name")]
    pub name: String,

    /// Output directory
    #[serde(default = "default_output_dir")]
    pub output: PathBuf,

    /// Source directory
    #[serde(default = "default_source_dir")]
    pub source: PathBuf,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: default_project_name(),
            output: default_output_dir(),
            source: default_source_dir(),
        }
    }
}

fn default_project_name() -> String {
    "my-game".to_string()
}

fn default_output_dir() -> PathBuf {
    PathBuf::from("./build/assets")
}

fn default_source_dir() -> PathBuf {
    PathBuf::from("./assets")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresetConfig {
    /// Maximum texture dimension
    #[serde(default)]
    pub texture_max_size: Option<u32>,

    /// Texture output format
    #[serde(default)]
    pub texture_format: Option<String>,

    /// Texture quality (0-100)
    #[serde(default)]
    pub texture_quality: Option<u8>,

    /// Audio output format
    #[serde(default)]
    pub audio_format: Option<String>,

    /// Audio quality (0-10)
    #[serde(default)]
    pub audio_quality: Option<u8>,

    /// Enable texture compression
    #[serde(default)]
    pub compress_textures: Option<bool>,

    /// Generate mipmaps
    #[serde(default)]
    pub generate_mipmaps: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleConfig {
    /// Output format
    #[serde(default)]
    pub format: Option<String>,

    /// Generate sprite atlas
    #[serde(default)]
    pub atlas: Option<bool>,

    /// Trim transparent pixels
    #[serde(default)]
    pub trim: Option<bool>,

    /// Generate mipmaps
    #[serde(default)]
    pub mipmap: Option<bool>,

    /// Apply Draco compression (for 3D models)
    #[serde(default)]
    pub draco: Option<bool>,

    /// Apply meshopt compression (for 3D models)
    #[serde(default)]
    pub meshopt: Option<bool>,

    /// Normalize audio volume
    #[serde(default)]
    pub normalize: Option<bool>,

    /// Quality setting (0-100)
    #[serde(default)]
    pub quality: Option<u8>,

    /// Maximum dimension
    #[serde(default)]
    pub max_size: Option<u32>,

    /// Custom output path pattern
    #[serde(default)]
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,

    /// Cache directory
    #[serde(default = "default_cache_dir")]
    pub directory: PathBuf,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: default_cache_enabled(),
            directory: default_cache_dir(),
        }
    }
}

fn default_cache_enabled() -> bool {
    true
}

fn default_cache_dir() -> PathBuf {
    PathBuf::from(".asset-forge-cache")
}

impl Config {
    /// Create a default configuration with sensible presets
    pub fn with_defaults() -> Self {
        let mut config = Config::default();

        // Mobile preset
        config.presets.insert(
            "mobile".to_string(),
            PresetConfig {
                texture_max_size: Some(1024),
                texture_format: Some("ktx2".to_string()),
                texture_quality: Some(75),
                audio_format: Some("ogg".to_string()),
                audio_quality: Some(6),
                compress_textures: Some(true),
                generate_mipmaps: Some(true),
            },
        );

        // Desktop preset
        config.presets.insert(
            "desktop".to_string(),
            PresetConfig {
                texture_max_size: Some(4096),
                texture_format: Some("png".to_string()),
                texture_quality: Some(90),
                audio_format: Some("wav".to_string()),
                audio_quality: Some(10),
                compress_textures: Some(false),
                generate_mipmaps: Some(true),
            },
        );

        // Web preset
        config.presets.insert(
            "web".to_string(),
            PresetConfig {
                texture_max_size: Some(2048),
                texture_format: Some("webp".to_string()),
                texture_quality: Some(80),
                audio_format: Some("ogg".to_string()),
                audio_quality: Some(7),
                compress_textures: Some(true),
                generate_mipmaps: Some(false),
            },
        );

        config
    }

    /// Generate default TOML content
    pub fn default_toml() -> String {
        r#"[project]
name = "my-game"
output = "./build/assets"
source = "./assets"

[presets.mobile]
texture_max_size = 1024
texture_format = "png"  # Will use "ktx2" when KTX2 support is added in Phase 2
texture_quality = 75
audio_format = "ogg"
audio_quality = 6
compress_textures = true
generate_mipmaps = true

[presets.desktop]
texture_max_size = 4096
texture_format = "png"
texture_quality = 90
audio_format = "wav"
audio_quality = 10
compress_textures = false
generate_mipmaps = true

[presets.web]
texture_max_size = 2048
texture_format = "webp"
texture_quality = 80
audio_format = "ogg"
audio_quality = 7
compress_textures = true
generate_mipmaps = false

[rules]
# Sprite atlas rules
# "sprites/*.png" = { atlas = true, trim = true }

# Texture rules
# "textures/*.png" = { format = "ktx2", mipmap = true }

# Model rules
# "models/*.gltf" = { draco = true, meshopt = true }

# Audio rules
# "audio/*.wav" = { format = "ogg", normalize = true }

[cache]
enabled = true
directory = ".asset-forge-cache"
"#
        .to_string()
    }
}
