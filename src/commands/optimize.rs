use anyhow::{Context, Result};
use console::style;
use std::path::PathBuf;

use crate::cli::OptimizeOptions;
use crate::processors::{process_image, AssetType, ImageProcessorConfig};

pub fn run(input: PathBuf, options: OptimizeOptions) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input file does not exist: {}", input.display());
    }

    let asset_type = AssetType::from_path(&input);

    match asset_type {
        AssetType::Image => optimize_image(&input, &options),
        AssetType::Model => {
            println!(
                "{} 3D model optimization is coming in Phase 2",
                style("!").yellow().bold()
            );
            Ok(())
        }
        AssetType::Audio => {
            println!(
                "{} Audio optimization is coming in Phase 2",
                style("!").yellow().bold()
            );
            Ok(())
        }
        AssetType::Unknown => {
            anyhow::bail!(
                "Unknown file type: {}. Supported types: images (.png, .jpg, .webp), models (.gltf, .glb), audio (.wav, .mp3, .ogg)",
                input.display()
            );
        }
    }
}

fn optimize_image(input: &PathBuf, options: &OptimizeOptions) -> Result<()> {
    let output = options.output.clone().unwrap_or_else(|| {
        if let Some(format) = &options.format {
            input.with_extension(format.to_string())
        } else {
            input.clone()
        }
    });

    println!(
        "{} Optimizing image: {}",
        style("→").blue().bold(),
        input.display()
    );

    let config = ImageProcessorConfig {
        output_format: options.format,
        quality: options.quality,
        max_size: None,
        generate_mipmaps: options.mipmap,
    };

    let stats = process_image(input, &output, &config)
        .with_context(|| format!("Failed to optimize image: {}", input.display()))?;

    // Print results
    println!(
        "{} Optimized: {} → {}",
        style("✓").green().bold(),
        style(format_size(stats.original_size)).dim(),
        style(format_size(stats.output_size)).green()
    );

    let reduction = stats.size_reduction_percent();
    if reduction > 0.0 {
        println!(
            "  {} size reduction ({} saved)",
            style(format!("{:.1}%", reduction)).green(),
            style(format_size(stats.original_size - stats.output_size)).green()
        );
    } else if reduction < 0.0 {
        println!(
            "  {} File size increased by {:.1}%",
            style("!").yellow().bold(),
            -reduction
        );
    }

    println!(
        "  Processed in {}",
        style(format!("{:.2}s", stats.processing_time_ms as f64 / 1000.0)).dim()
    );

    if output != *input {
        println!("  Output: {}", style(output.display()).cyan());
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;

    if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
