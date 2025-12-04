use anyhow::Result;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;

use crate::cli::{BuildOptions, OutputFormat, PlatformPreset, QualityPreset};
use crate::config::{find_and_load_config, load_config, Config, PresetConfig};
use crate::processors::{
    process_image, process_audio, process_model,
    AssetType, ImageProcessorConfig, AudioConfig, AudioFormat, ModelConfig,
    BuildCache, hash_config,
};

pub fn run(input: PathBuf, options: BuildOptions) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input directory does not exist: {}", input.display());
    }

    if !input.is_dir() {
        anyhow::bail!("Input path is not a directory: {}", input.display());
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
    let preset_config = get_preset_config(&options.preset, &config);

    println!(
        "{} Building assets from: {}",
        style("→").blue().bold(),
        input.display()
    );
    println!("  Output directory: {}", style(output_dir.display()).cyan());

    if let Some(preset) = &options.preset {
        println!("  Platform preset: {}", style(preset).cyan());
    }

    if options.dry_run {
        println!("  {}", style("(Dry run - no files will be processed)").yellow());
    }

    println!();

    // Collect all files to process
    let files: Vec<PathBuf> = WalkDir::new(&input)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .filter(|p| AssetType::from_path(p) != AssetType::Unknown)
        .collect();

    if files.is_empty() {
        println!("{} No supported asset files found", style("!").yellow().bold());
        return Ok(());
    }

    println!("Found {} asset files to process", style(files.len()).cyan());

    if options.dry_run {
        for file in &files {
            let relative = file.strip_prefix(&input).unwrap_or(file);
            let output_path = output_dir.join(relative);
            println!(
                "  {} → {}",
                style(file.display()).dim(),
                style(output_path.display()).green()
            );
        }
        return Ok(());
    }

    // Create progress bar
    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Track statistics
    let total_original = Arc::new(AtomicU64::new(0));
    let total_output = Arc::new(AtomicU64::new(0));
    let processed_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));

    // Configure parallelism
    let num_jobs = options.jobs.unwrap_or_else(num_cpus::get);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_jobs)
        .build()?;

    // Collect errors for later display
    let errors_list: Arc<Mutex<Vec<(PathBuf, String)>>> =
        Arc::new(Mutex::new(Vec::new()));

    // Load build cache for incremental builds
    let cache_dir = output_dir.join(".cache");
    let cache = Arc::new(Mutex::new(BuildCache::load(&cache_dir).unwrap_or_default()));
    let skipped_count = Arc::new(AtomicU64::new(0));
    let force_rebuild = options.force;

    // Process files in parallel
    let errors_clone = errors_list.clone();
    let cache_clone = cache.clone();
    let skipped_clone = skipped_count.clone();
    pool.install(|| {
        files.par_iter().for_each(|file| {
            let relative = file.strip_prefix(&input).unwrap_or(file);
            let output_path = output_dir.join(relative);

            // Check cache for incremental builds (skip if --force is used)
            let config_hash = compute_config_hash(&preset_config);
            let needs_rebuild = force_rebuild || cache_clone.lock().unwrap()
                .needs_rebuild(file, config_hash)
                .unwrap_or(true);

            if !needs_rebuild {
                skipped_clone.fetch_add(1, Ordering::Relaxed);
                pb.inc(1);
                return;
            }

            let result = process_file(file, &output_path, &preset_config);

            match result {
                Ok(Some((orig, out))) => {
                    total_original.fetch_add(orig, Ordering::Relaxed);
                    total_output.fetch_add(out, Ordering::Relaxed);
                    processed_count.fetch_add(1, Ordering::Relaxed);

                    // Update cache
                    let _ = cache_clone.lock().unwrap()
                        .update(file, &output_path, config_hash);
                }
                Ok(None) => {
                    // Skipped (e.g., unsupported type)
                }
                Err(e) => {
                    errors_clone.lock().unwrap().push((file.clone(), e.to_string()));
                    error_count.fetch_add(1, Ordering::Relaxed);
                }
            }

            pb.inc(1);
        });
    });

    pb.finish_and_clear();

    // Save cache
    {
        let mut cache_guard = cache.lock().unwrap();
        cache_guard.cleanup();
        let _ = cache_guard.save(&cache_dir);
    }

    // Print summary
    let processed = processed_count.load(Ordering::Relaxed);
    let errors = error_count.load(Ordering::Relaxed);
    let skipped = skipped_count.load(Ordering::Relaxed);
    let orig_size = total_original.load(Ordering::Relaxed);
    let out_size = total_output.load(Ordering::Relaxed);

    println!();
    println!("{} Build complete!", style("✓").green().bold());
    println!("  Files processed: {}", style(processed).green());
    if skipped > 0 {
        println!("  Files skipped (cached): {}", style(skipped).dim());
    }

    if errors > 0 {
        println!("  Errors: {}", style(errors).red());
        let error_list = errors_list.lock().unwrap();
        for (path, error) in error_list.iter().take(10) {
            println!(
                "    {} {}: {}",
                style("✗").red(),
                path.display(),
                error
            );
        }
        if error_list.len() > 10 {
            println!("    ... and {} more errors", error_list.len() - 10);
        }
    }

    if orig_size > 0 {
        let reduction = (1.0 - out_size as f64 / orig_size as f64) * 100.0;
        println!(
            "  Total size: {} → {} ({:.1}% reduction)",
            style(format_size(orig_size)).dim(),
            style(format_size(out_size)).green(),
            reduction
        );
    }

    println!("  Output: {}", style(output_dir.display()).cyan());

    Ok(())
}

fn process_file(
    input: &Path,
    output: &Path,
    preset: &PresetConfig,
) -> Result<Option<(u64, u64)>> {
    let asset_type = AssetType::from_path(input);

    match asset_type {
        AssetType::Image => {
            // Create output directory
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let image_config = ImageProcessorConfig {
                output_format: preset
                    .texture_format
                    .as_ref()
                    .and_then(|f| match f.as_str() {
                        "png" => Some(OutputFormat::Png),
                        "jpeg" | "jpg" => Some(OutputFormat::Jpeg),
                        "webp" => Some(OutputFormat::Webp),
                        "ktx2" => Some(OutputFormat::Ktx2),
                        _ => None,
                    }),
                quality: QualityPreset::Balanced,
                max_size: preset.texture_max_size,
                generate_mipmaps: preset.generate_mipmaps.unwrap_or(false),
            };

            let stats = process_image(input, output, &image_config)?;
            Ok(Some((stats.original_size, stats.output_size)))
        }
        AssetType::Audio => {
            // Process audio with configured format
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let output_format = preset
                .audio_format
                .as_ref()
                .map(|f| match f.as_str() {
                    "ogg" => AudioFormat::Ogg,
                    "wav" => AudioFormat::Wav,
                    _ => AudioFormat::Ogg,
                })
                .unwrap_or(AudioFormat::Ogg);

            // Map audio quality (1-10 scale) to vorbis quality (0.0-1.0)
            let quality = preset.audio_quality
                .map(|q| q as f32 / 10.0)
                .unwrap_or(0.5);

            let audio_config = AudioConfig {
                output_format,
                quality,
                sample_rate: None, // Keep original sample rate
                normalize: false,
            };

            // Adjust output extension based on format
            let output = match output_format {
                AudioFormat::Ogg => output.with_extension("ogg"),
                AudioFormat::Wav => output.with_extension("wav"),
            };

            let stats = process_audio(input, &output, &audio_config)?;
            Ok(Some((stats.original_size, stats.output_size)))
        }
        AssetType::Model => {
            // Process glTF/GLB models
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let ext = input.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());

            // Only process glTF/GLB files, copy others
            match ext.as_deref() {
                Some("gltf" | "glb") => {
                    let model_config = ModelConfig::default();
                    let output = output.with_extension("glb");
                    let stats = process_model(input, &output, &model_config)?;
                    Ok(Some((stats.original_size, stats.output_size)))
                }
                _ => {
                    // Copy unsupported model formats as-is
                    std::fs::copy(input, output)?;
                    let size = std::fs::metadata(output)?.len();
                    Ok(Some((size, size)))
                }
            }
        }
        AssetType::Unknown => Ok(None),
    }
}

/// Compute a hash of the preset configuration for cache invalidation
fn compute_config_hash(preset: &PresetConfig) -> u64 {
    hash_config(preset).unwrap_or(0)
}

fn get_preset_config(preset: &Option<PlatformPreset>, config: &Option<Config>) -> PresetConfig {
    if let Some(preset_name) = preset {
        if let Some(cfg) = config {
            let name = preset_name.to_string();
            if let Some(preset_cfg) = cfg.presets.get(&name) {
                return preset_cfg.clone();
            }
        }

        // Default presets
        match preset_name {
            PlatformPreset::Mobile => PresetConfig {
                texture_max_size: Some(1024),
                texture_format: Some("png".to_string()), // Use PNG for now, KTX2 in Phase 2
                texture_quality: Some(75),
                audio_format: Some("ogg".to_string()),
                audio_quality: Some(6),
                compress_textures: Some(true),
                generate_mipmaps: Some(true),
            },
            PlatformPreset::Desktop => PresetConfig {
                texture_max_size: Some(4096),
                texture_format: Some("png".to_string()),
                texture_quality: Some(90),
                audio_format: Some("wav".to_string()),
                audio_quality: Some(10),
                compress_textures: Some(false),
                generate_mipmaps: Some(true),
            },
            PlatformPreset::Web => PresetConfig {
                texture_max_size: Some(2048),
                texture_format: Some("webp".to_string()),
                texture_quality: Some(80),
                audio_format: Some("ogg".to_string()),
                audio_quality: Some(7),
                compress_textures: Some(true),
                generate_mipmaps: Some(false),
            },
        }
    } else {
        PresetConfig::default()
    }
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
