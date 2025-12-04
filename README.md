# Asset Forge

A unified game asset processing CLI tool written in Rust. One-stop solution for game asset optimization.

## Features

### Image Processing
- PNG optimization using oxipng (multi-threaded, lossless)
- JPEG/WebP conversion with quality control
- KTX2/Basis Universal GPU texture compression (UASTC/ETC1S)
- Automatic resizing with max dimension limits
- Mipmap generation

### Sprite Atlas Generation
- Automatic texture packing
- JSON metadata output (compatible with game engines)
- Configurable padding and trimming

### 3D Model Processing
- glTF/GLB optimization and validation
- Meshopt compression (vertex cache, overdraw, fetch optimization)
- Mesh simplification for LOD generation
- Buffer encoding for smaller file sizes

### Audio Processing
- WAV/MP3/FLAC/OGG decoding (via Symphonia)
- OGG Vorbis encoding with quality VBR
- WAV output (16-bit PCM)
- Audio normalization and resampling

### Build System
- Incremental builds with content hashing
- Platform presets (mobile, desktop, web)
- Parallel processing with configurable threads
- Watch mode for development

### Configuration
- TOML configuration files
- Glob pattern rules for automatic processing
- CI/CD friendly

## Installation

### From Source
```bash
git clone https://github.com/esengine/asset-forge.git
cd asset-forge
cargo build --release
```

The binary will be at `target/release/asset-forge` (or `asset-forge.exe` on Windows).

## Quick Start

### Initialize Configuration
```bash
asset-forge init
```

This creates an `asset-forge.toml` configuration file in your current directory.

### Optimize Images
```bash
# Optimize PNG with high quality compression
asset-forge optimize hero.png --quality high

# Convert to WebP
asset-forge optimize hero.png --format webp --output hero.webp

# Convert to KTX2 (GPU compressed texture)
asset-forge optimize hero.png --format ktx2
```

### Process 3D Models
```bash
# View model information
asset-forge model character.glb --info

# Optimize with meshopt compression
asset-forge model character.glb --optimize --compress

# Generate LOD levels
asset-forge model character.glb --lod --lod-count 3

# Full optimization pipeline
asset-forge model character.glb -o optimized.glb --optimize --compress --lod
```

### Build All Assets
```bash
# Process all assets in a directory
asset-forge build ./assets --output ./build

# Use platform preset
asset-forge build ./assets --preset mobile
asset-forge build ./assets --preset desktop
asset-forge build ./assets --preset web

# Force rebuild (ignore cache)
asset-forge build ./assets --force

# Dry run to see what would be processed
asset-forge build ./assets --dry-run
```

### Generate Sprite Atlas
```bash
# Basic atlas generation
asset-forge atlas ./sprites --output atlas.png

# With custom settings
asset-forge atlas ./sprites --output atlas.png --max-width 4096 --padding 4 --trim
```

### Watch Mode
```bash
# Watch for changes and auto-process
asset-forge watch ./assets --output ./build --preset web
```

## Configuration

### asset-forge.toml
```toml
[project]
name = "my-game"
output = "./build/assets"
source = "./assets"

[presets.mobile]
texture_max_size = 1024
texture_format = "png"
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

[presets.web]
texture_max_size = 2048
texture_format = "webp"
texture_quality = 80
audio_format = "ogg"
audio_quality = 7

[rules]
# Auto-process files matching patterns
"sprites/*.png" = { atlas = true, trim = true }
"textures/*.png" = { format = "ktx2", mipmap = true }
"models/*.gltf" = { optimize = true, compress = true }
"audio/*.wav" = { format = "ogg", normalize = true }

[cache]
enabled = true
directory = ".asset-forge-cache"
```

## CLI Reference

### Global Options
```
-v, --verbose    Enable verbose output
-q, --quiet      Suppress all output except errors
-h, --help       Print help
-V, --version    Print version
```

### Commands

#### `init`
Initialize a new configuration file.
```bash
asset-forge init [--force]
```

#### `optimize`
Optimize a single asset file.
```bash
asset-forge optimize <INPUT> [OPTIONS]

Options:
  -o, --output <PATH>     Output file path
  -f, --format <FORMAT>   Output format (png, jpeg, webp, ktx2)
  -q, --quality <PRESET>  Quality preset (fast, balanced, high, ultra)
      --mipmap            Generate mipmaps
```

#### `build`
Build and process all assets in a directory.
```bash
asset-forge build <INPUT> [OPTIONS]

Options:
  -o, --output <PATH>     Output directory
  -p, --preset <PRESET>   Platform preset (mobile, desktop, web)
  -c, --config <PATH>     Configuration file path
      --force             Force rebuild all assets (ignore cache)
  -j, --jobs <N>          Number of parallel jobs
      --dry-run           Show what would be processed
```

#### `atlas`
Generate a sprite atlas from multiple images.
```bash
asset-forge atlas <INPUT> [OPTIONS]

Options:
  -o, --output <PATH>     Output atlas image path
      --json <PATH>       Output JSON metadata path
      --max-width <N>     Maximum atlas width (default: 2048)
      --max-height <N>    Maximum atlas height (default: 2048)
      --padding <N>       Padding between sprites (default: 2)
      --trim              Trim transparent pixels
  -f, --format <FORMAT>   Output format
```

#### `model`
Optimize a 3D model (glTF/GLB).
```bash
asset-forge model <INPUT> [OPTIONS]

Options:
  -o, --output <PATH>     Output file path
      --optimize          Enable mesh optimization (vertex cache, overdraw, fetch)
      --compress          Enable meshopt buffer encoding/compression
      --lod               Generate LOD levels
      --lod-count <N>     Number of LOD levels (1-4, default: 3)
      --lod-ratio <R>     Target ratio per LOD level (0.1-0.9, default: 0.5)
      --info              Show model information without processing
```

#### `watch`
Watch for file changes and automatically process assets.
```bash
asset-forge watch <INPUT> [OPTIONS]

Options:
  -o, --output <PATH>     Output directory
  -c, --config <PATH>     Configuration file path
  -p, --preset <PRESET>   Platform preset
      --debounce <MS>     Debounce delay in milliseconds (default: 300)
```

#### `audio`
Process audio files (transcode, normalize, resample).
```bash
asset-forge audio <INPUT> [OPTIONS]

Options:
  -o, --output <PATH>     Output file path
  -f, --format <FORMAT>   Output format (ogg, wav)
  -q, --quality <N>       Quality level 1-10 (default: 5, for OGG)
      --sample-rate <HZ>  Target sample rate
      --normalize         Normalize audio volume
      --info              Show audio information without processing
```

#### `info`
Show information about an asset file.
```bash
asset-forge info <INPUT>

# Displays:
# - File size and type
# - Image: dimensions, color type, compression ratio
# - Model: meshes, vertices, materials, animations
# - Audio: channels, sample rate, duration, bitrate
```

#### `clean`
Clear the build cache.
```bash
asset-forge clean [OPTIONS]

Options:
  -c, --cache-dir <PATH>  Cache directory (default: .cache in output dir)
      --all               Also remove output directory
```

## Quality Presets

| Preset | Description | Use Case |
|--------|-------------|----------|
| `fast` | Fastest processing, larger files | Development builds |
| `balanced` | Good balance of speed and size | Default |
| `high` | Better compression, slower | Release builds |
| `ultra` | Maximum compression, slowest | Final distribution |

## Platform Presets

| Preset | Max Texture | Format | Audio | Description |
|--------|-------------|--------|-------|-------------|
| `mobile` | 1024px | PNG | OGG | Optimized for mobile devices |
| `desktop` | 4096px | PNG | WAV | High quality for desktop |
| `web` | 2048px | WebP | OGG | Optimized for web delivery |

## Examples

### Game Asset Pipeline
```bash
# Initialize project
asset-forge init

# During development - watch mode
asset-forge watch ./assets --output ./build --preset desktop

# For mobile release
asset-forge build ./assets --preset mobile --output ./build/mobile

# For web release
asset-forge build ./assets --preset web --output ./build/web
```

### Model Optimization
```bash
# Check model stats
asset-forge model character.glb --info

# Full optimization with LOD
asset-forge model character.glb \
  --optimize \
  --compress \
  --lod \
  --lod-count 3 \
  -o character_optimized.glb
```

### Creating Sprite Sheets
```bash
# Create atlas from character sprites
asset-forge atlas ./sprites/character --output atlas/character.png --trim

# Create UI atlas with WebP output
asset-forge atlas ./sprites/ui --output atlas/ui.webp --padding 4 --format webp
```

### CI/CD Integration
```bash
# In your build script
asset-forge build ./assets \
  --preset mobile \
  --output ./build/assets \
  --jobs 4

# Check exit code for CI
if asset-forge build ./assets --preset web; then
  echo "Build successful"
fi
```

## Supported Formats

### Input
| Type | Formats |
|------|---------|
| Images | PNG, JPEG, WebP, BMP, GIF, TIFF |
| Audio | WAV, MP3, OGG, FLAC |
| Models | glTF, GLB |

### Output
| Type | Formats |
|------|---------|
| Images | PNG, JPEG, WebP, KTX2 (Basis Universal) |
| Audio | OGG (Vorbis), WAV |
| Models | GLB |

## Dependencies

Asset Forge uses several excellent Rust crates:

- **Image Processing**: [image](https://crates.io/crates/image), [oxipng](https://crates.io/crates/oxipng), [basis-universal](https://crates.io/crates/basis-universal)
- **3D Models**: [gltf](https://crates.io/crates/gltf), [meshopt](https://crates.io/crates/meshopt)
- **Audio**: [symphonia](https://crates.io/crates/symphonia), [vorbis_rs](https://crates.io/crates/vorbis_rs)
- **CLI**: [clap](https://crates.io/crates/clap)

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Roadmap

- [x] Phase 1: MVP (PNG optimization, sprite atlas, basic CLI)
- [x] Phase 2: KTX2/Basis Universal, glTF processing, audio transcoding
- [x] Phase 3: Meshopt compression, LOD generation, enhanced watch mode
- [x] Phase 4: Standalone audio/info/clean commands
- [ ] Phase 5: Draco compression, GUI version, engine plugins (Bevy, Fyrox)
