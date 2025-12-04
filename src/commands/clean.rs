use anyhow::Result;
use console::style;
use std::path::PathBuf;

use crate::config::find_and_load_config;

pub fn run(cache_dir: Option<PathBuf>, all: bool) -> Result<()> {
    // Try to load config to find default directories
    let config = find_and_load_config().ok().flatten();

    // Determine cache directory
    let cache_path = cache_dir
        .or_else(|| {
            config.as_ref().map(|c| c.project.output.join(".cache"))
        })
        .unwrap_or_else(|| PathBuf::from("./build/.cache"));

    // Determine output directory (only used with --all)
    let output_path = config
        .as_ref()
        .map(|c| c.project.output.clone())
        .unwrap_or_else(|| PathBuf::from("./build"));

    println!("{} Cleaning build artifacts", style("🧹").blue().bold());

    // Clean cache directory
    if cache_path.exists() {
        let cache_size = dir_size(&cache_path).unwrap_or(0);
        std::fs::remove_dir_all(&cache_path)?;
        println!(
            "  {} Removed cache: {} ({})",
            style("✓").green(),
            cache_path.display(),
            format_size(cache_size)
        );
    } else {
        println!(
            "  {} Cache not found: {}",
            style("-").dim(),
            cache_path.display()
        );
    }

    // Clean output directory if --all is specified
    if all {
        if output_path.exists() {
            let output_size = dir_size(&output_path).unwrap_or(0);
            std::fs::remove_dir_all(&output_path)?;
            println!(
                "  {} Removed output: {} ({})",
                style("✓").green(),
                output_path.display(),
                format_size(output_size)
            );
        } else {
            println!(
                "  {} Output not found: {}",
                style("-").dim(),
                output_path.display()
            );
        }
    }

    // Also check for common cache locations
    let common_cache_paths = [
        PathBuf::from(".asset-forge-cache"),
        PathBuf::from(".cache"),
    ];

    for path in &common_cache_paths {
        if path.exists() && path != &cache_path {
            let size = dir_size(path).unwrap_or(0);
            std::fs::remove_dir_all(path)?;
            println!(
                "  {} Removed: {} ({})",
                style("✓").green(),
                path.display(),
                format_size(size)
            );
        }
    }

    println!();
    println!("{} Clean complete!", style("✓").green().bold());

    Ok(())
}

fn dir_size(path: &PathBuf) -> Result<u64> {
    let mut size = 0;

    if path.is_file() {
        return Ok(std::fs::metadata(path)?.len());
    }

    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            size += entry.metadata()?.len();
        }
    }

    Ok(size)
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
