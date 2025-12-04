use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "asset-forge",
    author = "esengine",
    version,
    about = "A unified game asset processing CLI tool",
    long_about = "Asset Forge - One-stop solution for game asset optimization.\n\n\
                  Supports image compression, texture atlas generation, 3D model optimization,\n\
                  audio processing, and more."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new asset-forge.toml configuration file
    Init {
        /// Overwrite existing configuration
        #[arg(short, long)]
        force: bool,
    },

    /// Optimize a single asset file
    Optimize {
        /// Input file path
        input: PathBuf,

        #[command(flatten)]
        options: OptimizeOptions,
    },

    /// Build and process all assets in a directory
    Build {
        /// Input directory path
        input: PathBuf,

        #[command(flatten)]
        options: BuildOptions,
    },

    /// Generate a sprite atlas from multiple images
    Atlas {
        /// Input directory containing sprites
        input: PathBuf,

        #[command(flatten)]
        options: AtlasOptions,
    },

    /// Watch for file changes and automatically process assets
    Watch {
        /// Directory to watch
        input: PathBuf,

        #[command(flatten)]
        options: WatchOptions,
    },

    /// Optimize a 3D model (glTF/GLB)
    Model {
        /// Input model file path
        input: PathBuf,

        #[command(flatten)]
        options: ModelOptions,
    },

    /// Process audio files (transcode, normalize, resample)
    Audio {
        /// Input audio file path
        input: PathBuf,

        #[command(flatten)]
        options: AudioOptions,
    },

    /// Show information about an asset file
    Info {
        /// Input file path
        input: PathBuf,
    },

    /// Clear the build cache
    Clean {
        /// Cache directory (default: .cache in output dir)
        #[arg(short, long)]
        cache_dir: Option<PathBuf>,

        /// Also remove output directory
        #[arg(long)]
        all: bool,
    },
}

#[derive(Args, Clone)]
pub struct OptimizeOptions {
    /// Output file path (default: overwrites input)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format
    #[arg(short, long)]
    pub format: Option<OutputFormat>,

    /// Quality preset
    #[arg(short, long, default_value = "balanced")]
    pub quality: QualityPreset,

    /// Generate mipmaps (for textures)
    #[arg(long)]
    pub mipmap: bool,
}

#[derive(Args, Clone)]
pub struct BuildOptions {
    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Platform preset (mobile, desktop, web)
    #[arg(short, long)]
    pub preset: Option<PlatformPreset>,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Force rebuild all assets (ignore cache)
    #[arg(long)]
    pub force: bool,

    /// Number of parallel jobs
    #[arg(short, long)]
    pub jobs: Option<usize>,

    /// Dry run - show what would be processed without actually processing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Clone)]
pub struct AtlasOptions {
    /// Output atlas image path
    #[arg(short, long, default_value = "atlas.png")]
    pub output: PathBuf,

    /// Output JSON metadata path
    #[arg(long)]
    pub json: Option<PathBuf>,

    /// Maximum atlas width
    #[arg(long, default_value = "2048")]
    pub max_width: u32,

    /// Maximum atlas height
    #[arg(long, default_value = "2048")]
    pub max_height: u32,

    /// Padding between sprites
    #[arg(long, default_value = "2")]
    pub padding: u32,

    /// Trim transparent pixels from sprites
    #[arg(long)]
    pub trim: bool,

    /// Output format for the atlas
    #[arg(short, long)]
    pub format: Option<OutputFormat>,
}

#[derive(Args, Clone)]
pub struct WatchOptions {
    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Platform preset
    #[arg(short, long)]
    pub preset: Option<PlatformPreset>,

    /// Debounce delay in milliseconds
    #[arg(long, default_value = "300")]
    pub debounce: u64,
}

#[derive(Args, Clone)]
pub struct ModelOptions {
    /// Output file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Enable mesh optimization (vertex cache, overdraw, fetch)
    #[arg(long, default_value = "true")]
    pub optimize: bool,

    /// Enable meshopt buffer encoding/compression
    #[arg(long)]
    pub compress: bool,

    /// Generate LOD levels
    #[arg(long)]
    pub lod: bool,

    /// Number of LOD levels to generate (1-4)
    #[arg(long, default_value = "3")]
    pub lod_count: u32,

    /// Target ratio for each LOD level (0.1-0.9)
    #[arg(long, default_value = "0.5")]
    pub lod_ratio: f32,

    /// Show model information without processing
    #[arg(long)]
    pub info: bool,
}

#[derive(Args, Clone)]
pub struct AudioOptions {
    /// Output file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format (ogg, wav)
    #[arg(short, long, default_value = "ogg")]
    pub format: AudioOutputFormat,

    /// Quality for OGG encoding (1-10, default: 5)
    #[arg(short, long, default_value = "5")]
    pub quality: u8,

    /// Target sample rate (e.g., 44100, 48000)
    #[arg(long)]
    pub sample_rate: Option<u32>,

    /// Normalize audio volume
    #[arg(long)]
    pub normalize: bool,

    /// Show audio information without processing
    #[arg(long)]
    pub info: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
pub enum AudioOutputFormat {
    #[default]
    Ogg,
    Wav,
}

impl std::fmt::Display for AudioOutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioOutputFormat::Ogg => write!(f, "ogg"),
            AudioOutputFormat::Wav => write!(f, "wav"),
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
pub enum OutputFormat {
    #[default]
    Png,
    Jpeg,
    Webp,
    Ktx2,
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
pub enum QualityPreset {
    /// Fastest processing, larger file size
    Fast,
    /// Balance between speed and size
    #[default]
    Balanced,
    /// Best compression, slower processing
    High,
    /// Maximum compression, slowest processing
    Ultra,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum PlatformPreset {
    /// Optimized for mobile devices (smaller textures, compressed formats)
    Mobile,
    /// Optimized for desktop (higher quality)
    Desktop,
    /// Optimized for web (WebP, smaller sizes)
    Web,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Png => write!(f, "png"),
            OutputFormat::Jpeg => write!(f, "jpeg"),
            OutputFormat::Webp => write!(f, "webp"),
            OutputFormat::Ktx2 => write!(f, "ktx2"),
        }
    }
}

impl std::fmt::Display for QualityPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityPreset::Fast => write!(f, "fast"),
            QualityPreset::Balanced => write!(f, "balanced"),
            QualityPreset::High => write!(f, "high"),
            QualityPreset::Ultra => write!(f, "ultra"),
        }
    }
}

impl std::fmt::Display for PlatformPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformPreset::Mobile => write!(f, "mobile"),
            PlatformPreset::Desktop => write!(f, "desktop"),
            PlatformPreset::Web => write!(f, "web"),
        }
    }
}
