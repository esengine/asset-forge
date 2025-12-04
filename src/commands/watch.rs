use anyhow::Result;
use console::style;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use crate::cli::{WatchOptions, PlatformPreset};
use crate::config::{find_and_load_config, load_config, PresetConfig};
use crate::processors::{
    process_image, process_audio, process_model,
    AssetType, ImageProcessorConfig, AudioConfig, AudioFormat, ModelConfig,
};

/// Watch statistics
struct WatchStats {
    processed: u64,
    errors: u64,
    skipped: u64,
    start_time: Instant,
}

impl WatchStats {
    fn new() -> Self {
        Self {
            processed: 0,
            errors: 0,
            skipped: 0,
            start_time: Instant::now(),
        }
    }

    fn print_summary(&self) {
        let elapsed = self.start_time.elapsed();
        println!();
        println!(
            "{} Watch session summary:",
            style("📊").blue().bold()
        );
        println!("  Duration: {:.1}s", elapsed.as_secs_f64());
        println!("  Processed: {}", style(self.processed).green());
        if self.errors > 0 {
            println!("  Errors: {}", style(self.errors).red());
        }
        if self.skipped > 0 {
            println!("  Skipped: {}", style(self.skipped).dim());
        }
    }
}

/// Debouncer to prevent duplicate processing
struct Debouncer {
    last_events: HashMap<PathBuf, Instant>,
    debounce_duration: Duration,
}

impl Debouncer {
    fn new(debounce_ms: u64) -> Self {
        Self {
            last_events: HashMap::new(),
            debounce_duration: Duration::from_millis(debounce_ms),
        }
    }

    fn should_process(&mut self, path: &Path) -> bool {
        let now = Instant::now();
        let path_buf = path.to_path_buf();

        if let Some(last_time) = self.last_events.get(&path_buf) {
            if now.duration_since(*last_time) < self.debounce_duration {
                return false;
            }
        }

        self.last_events.insert(path_buf, now);
        true
    }

    fn cleanup(&mut self) {
        let now = Instant::now();
        self.last_events.retain(|_, last_time| {
            now.duration_since(*last_time) < Duration::from_secs(60)
        });
    }
}

pub fn run(input: PathBuf, options: WatchOptions) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Watch directory does not exist: {}", input.display());
    }

    if !input.is_dir() {
        anyhow::bail!("Watch path is not a directory: {}", input.display());
    }

    // Load configuration
    let config = if let Some(config_path) = &options.config {
        Some(load_config(config_path)?)
    } else {
        find_and_load_config()?
    };

    // Determine output directory
    let output_dir = options
        .output
        .clone()
        .or_else(|| config.as_ref().map(|c| c.project.output.clone()))
        .unwrap_or_else(|| PathBuf::from("./build/assets"));

    // Get preset configuration
    let preset_config = get_preset_config(&options.preset);

    // Create output directory
    std::fs::create_dir_all(&output_dir)?;

    println!(
        "{} Watch mode started",
        style("👁").blue().bold()
    );
    println!("  Watching: {}", style(input.display()).cyan());
    println!("  Output: {}", style(output_dir.display()).cyan());
    if let Some(preset) = &options.preset {
        println!("  Preset: {}", style(preset).cyan());
    }
    println!("  Debounce: {}ms", options.debounce);
    println!();
    println!("  Press {} to stop", style("Ctrl+C").yellow());
    println!();
    println!("{}", style("─".repeat(50)).dim());
    println!();

    // Create a channel to receive the events
    let (tx, rx) = channel();

    // Create a watcher with proper config
    let watcher_config = Config::default()
        .with_poll_interval(Duration::from_millis(100));

    let mut watcher = RecommendedWatcher::new(tx, watcher_config)?;

    // Watch the directory
    watcher.watch(&input, RecursiveMode::Recursive)?;

    // Initialize debouncer and stats
    let mut debouncer = Debouncer::new(options.debounce);
    let mut stats = WatchStats::new();
    let mut cleanup_counter = 0u32;

    // Set up Ctrl+C handler
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    }).ok(); // Ignore if already set

    // Process events
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        // Use recv_timeout to allow checking the running flag
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(event)) => {
                process_event(&event, &input, &output_dir, &preset_config, &mut debouncer, &mut stats);
            }
            Ok(Err(e)) => {
                eprintln!(
                    "{} Watch error: {}",
                    style("⚠").yellow(),
                    e
                );
                // Continue watching - don't exit on recoverable errors
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Periodic cleanup
                cleanup_counter += 1;
                if cleanup_counter >= 120 { // Every ~60 seconds
                    debouncer.cleanup();
                    cleanup_counter = 0;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("{} Watcher disconnected", style("✗").red());
                break;
            }
        }
    }

    // Print summary on exit
    stats.print_summary();

    Ok(())
}

fn process_event(
    event: &Event,
    input_dir: &Path,
    output_dir: &Path,
    preset: &PresetConfig,
    debouncer: &mut Debouncer,
    stats: &mut WatchStats,
) {
    // Only process create and modify events
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {}
        _ => return,
    }

    for path in &event.paths {
        // Skip directories
        if !path.is_file() {
            continue;
        }

        // Check asset type
        let asset_type = AssetType::from_path(path);
        if asset_type == AssetType::Unknown {
            continue;
        }

        // Debounce check
        if !debouncer.should_process(path) {
            stats.skipped += 1;
            continue;
        }

        // Calculate output path
        let relative = path.strip_prefix(input_dir).unwrap_or(path);
        let output_path = output_dir.join(relative);

        // Print processing message
        let now = chrono_lite_time();
        println!(
            "{} [{}] {}",
            style("→").blue(),
            style(&now).dim(),
            path.file_name().unwrap_or_default().to_string_lossy()
        );

        // Process the asset
        let start = Instant::now();
        match process_asset(path, &output_path, preset) {
            Ok(size_info) => {
                let elapsed = start.elapsed();
                stats.processed += 1;
                println!(
                    "  {} {} ({}, {:.0}ms)",
                    style("✓").green(),
                    output_path.file_name().unwrap_or_default().to_string_lossy(),
                    size_info,
                    elapsed.as_secs_f64() * 1000.0
                );
            }
            Err(e) => {
                stats.errors += 1;
                eprintln!(
                    "  {} Error: {}",
                    style("✗").red(),
                    e
                );
            }
        }
    }
}

fn process_asset(input: &Path, output: &Path, preset: &PresetConfig) -> Result<String> {
    let asset_type = AssetType::from_path(input);

    // Create output directory
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let original_size = std::fs::metadata(input)?.len();

    match asset_type {
        AssetType::Image => {
            let config = ImageProcessorConfig {
                output_format: preset.texture_format.as_ref().and_then(|f| {
                    match f.as_str() {
                        "png" => Some(crate::cli::OutputFormat::Png),
                        "jpeg" | "jpg" => Some(crate::cli::OutputFormat::Jpeg),
                        "webp" => Some(crate::cli::OutputFormat::Webp),
                        "ktx2" => Some(crate::cli::OutputFormat::Ktx2),
                        _ => None,
                    }
                }),
                quality: crate::cli::QualityPreset::Balanced,
                max_size: preset.texture_max_size,
                generate_mipmaps: preset.generate_mipmaps.unwrap_or(false),
            };
            let stats = process_image(input, output, &config)?;
            Ok(format_size_change(stats.original_size, stats.output_size))
        }
        AssetType::Audio => {
            let output_format = preset.audio_format.as_ref()
                .map(|f| match f.as_str() {
                    "ogg" => AudioFormat::Ogg,
                    "wav" => AudioFormat::Wav,
                    _ => AudioFormat::Ogg,
                })
                .unwrap_or(AudioFormat::Ogg);

            let quality = preset.audio_quality
                .map(|q| q as f32 / 10.0)
                .unwrap_or(0.5);

            let audio_config = AudioConfig {
                output_format,
                quality,
                sample_rate: None,
                normalize: false,
            };

            // Adjust output extension
            let output = match output_format {
                AudioFormat::Ogg => output.with_extension("ogg"),
                AudioFormat::Wav => output.with_extension("wav"),
            };

            let stats = process_audio(input, &output, &audio_config)?;
            Ok(format_size_change(stats.original_size, stats.output_size))
        }
        AssetType::Model => {
            let ext = input.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());

            match ext.as_deref() {
                Some("gltf" | "glb") => {
                    let model_config = ModelConfig::default();
                    let output = output.with_extension("glb");
                    let stats = process_model(input, &output, &model_config)?;
                    Ok(format_size_change(stats.original_size, stats.output_size))
                }
                _ => {
                    // Copy unsupported model formats
                    std::fs::copy(input, output)?;
                    let output_size = std::fs::metadata(output)?.len();
                    Ok(format_size_change(original_size, output_size))
                }
            }
        }
        AssetType::Unknown => {
            anyhow::bail!("Unknown asset type");
        }
    }
}

fn get_preset_config(preset: &Option<PlatformPreset>) -> PresetConfig {
    match preset {
        Some(PlatformPreset::Mobile) => PresetConfig {
            texture_max_size: Some(1024),
            texture_format: Some("png".to_string()),
            texture_quality: Some(75),
            audio_format: Some("ogg".to_string()),
            audio_quality: Some(6),
            compress_textures: Some(true),
            generate_mipmaps: Some(true),
        },
        Some(PlatformPreset::Desktop) => PresetConfig {
            texture_max_size: Some(4096),
            texture_format: Some("png".to_string()),
            texture_quality: Some(90),
            audio_format: Some("wav".to_string()),
            audio_quality: Some(10),
            compress_textures: Some(false),
            generate_mipmaps: Some(true),
        },
        Some(PlatformPreset::Web) => PresetConfig {
            texture_max_size: Some(2048),
            texture_format: Some("webp".to_string()),
            texture_quality: Some(80),
            audio_format: Some("ogg".to_string()),
            audio_quality: Some(7),
            compress_textures: Some(true),
            generate_mipmaps: Some(false),
        },
        None => PresetConfig::default(),
    }
}

fn format_size_change(original: u64, output: u64) -> String {
    let reduction = if original > 0 {
        (1.0 - output as f64 / original as f64) * 100.0
    } else {
        0.0
    };

    if reduction > 0.0 {
        format!("{} → {} ({:.1}% smaller)",
            format_size(original),
            format_size(output),
            reduction
        )
    } else if reduction < 0.0 {
        format!("{} → {} ({:.1}% larger)",
            format_size(original),
            format_size(output),
            -reduction
        )
    } else {
        format!("{}", format_size(output))
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;

    if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Simple time formatting without external crate
fn chrono_lite_time() -> String {
    use std::time::SystemTime;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();

    let total_secs = now.as_secs();
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
