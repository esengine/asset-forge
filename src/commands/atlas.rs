use anyhow::Result;
use console::style;
use std::path::PathBuf;

use crate::cli::AtlasOptions;
use crate::processors::{generate_atlas, save_atlas_metadata, AtlasConfig};

pub fn run(input: PathBuf, options: AtlasOptions) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input directory does not exist: {}", input.display());
    }

    if !input.is_dir() {
        anyhow::bail!("Input path is not a directory: {}", input.display());
    }

    println!(
        "{} Generating sprite atlas from: {}",
        style("→").blue().bold(),
        input.display()
    );

    let config = AtlasConfig {
        max_width: options.max_width,
        max_height: options.max_height,
        padding: options.padding,
        trim: options.trim,
        allow_rotation: false,
    };

    let result = generate_atlas(&input, &options.output, &config)?;

    // Save metadata JSON if requested
    let json_path = options.json.unwrap_or_else(|| {
        options.output.with_extension("json")
    });

    save_atlas_metadata(&result.metadata, &json_path)?;

    // Print results
    println!(
        "{} Atlas generated successfully!",
        style("✓").green().bold()
    );
    println!();
    println!("  Atlas image: {}", style(options.output.display()).cyan());
    println!("  Metadata: {}", style(json_path.display()).cyan());
    println!();
    println!("  Dimensions: {}x{}", result.metadata.width, result.metadata.height);
    println!("  Sprites packed: {}", style(result.metadata.frames.len()).green());
    println!();
    println!(
        "  Original total: {}",
        style(format_size(result.stats.original_size)).dim()
    );
    println!(
        "  Atlas size: {}",
        style(format_size(result.stats.output_size)).green()
    );

    let reduction = result.stats.size_reduction_percent();
    if reduction > 0.0 {
        println!(
            "  Size reduction: {}",
            style(format!("{:.1}%", reduction)).green()
        );
    }

    println!(
        "  Processing time: {}",
        style(format!("{:.2}s", result.stats.processing_time_ms as f64 / 1000.0)).dim()
    );

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
