use anyhow::Result;
use console::style;
use std::path::PathBuf;
use std::time::Instant;

use crate::cli::ModelOptions;
use crate::processors::{
    get_model_info, process_model, estimate_lod_levels,
    ModelConfig, detect_model_format,
};

pub fn run(input: PathBuf, options: ModelOptions) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input file does not exist: {}", input.display());
    }

    // Detect model format
    let format = detect_model_format(&input)
        .ok_or_else(|| anyhow::anyhow!("Unsupported model format: {}", input.display()))?;

    // Check if it's a supported format
    match format {
        crate::processors::ModelFormat::GlTF | crate::processors::ModelFormat::GLB => {}
        _ => {
            anyhow::bail!(
                "Only glTF/GLB formats are supported for optimization. Found: {}",
                format
            );
        }
    }

    // Info-only mode
    if options.info {
        return print_model_info(&input);
    }

    // Determine output path
    let output = options.output.unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default();
        let default_dir = PathBuf::from(".");
        let parent = input.parent().unwrap_or(&default_dir);
        parent.join(format!("{}_optimized.glb", stem.to_string_lossy()))
    });

    println!(
        "{} Processing model: {}",
        style("→").blue().bold(),
        input.display()
    );
    println!("  Format: {}", style(format).cyan());

    // Get and display model info
    let info = get_model_info(&input)?;
    println!(
        "  Meshes: {}, Vertices: {}, Indices: {}",
        style(info.meshes).cyan(),
        style(info.total_vertices).cyan(),
        style(info.total_indices).cyan()
    );

    if info.materials > 0 {
        println!("  Materials: {}", style(info.materials).cyan());
    }
    if info.textures > 0 {
        println!("  Textures: {}", style(info.textures).cyan());
    }
    if info.animations > 0 {
        println!("  Animations: {}", style(info.animations).cyan());
    }

    // Build config
    let config = ModelConfig {
        optimize_meshes: options.optimize,
        encode_buffers: options.compress,
        generate_lods: options.lod,
        lod_count: options.lod_count.clamp(1, 4),
        lod_ratio: options.lod_ratio.clamp(0.1, 0.9),
        output_glb: true,
    };

    // Show what optimizations will be applied
    println!();
    println!("{} Optimizations:", style("⚙").blue().bold());
    if config.optimize_meshes {
        println!("  {} Vertex cache optimization", style("✓").green());
        println!("  {} Overdraw optimization", style("✓").green());
        println!("  {} Vertex fetch optimization", style("✓").green());
    }
    if config.encode_buffers {
        println!("  {} Meshopt buffer compression", style("✓").green());
    }
    if config.generate_lods {
        println!(
            "  {} LOD generation ({} levels, {}% ratio)",
            style("✓").green(),
            config.lod_count,
            (config.lod_ratio * 100.0) as u32
        );

        // Show estimated LOD levels
        let lod_estimates = estimate_lod_levels(&info);
        for est in &lod_estimates {
            println!(
                "    LOD {}: ~{} triangles (distance: {})",
                est.level,
                est.estimated_triangles,
                est.suggested_distance
            );
        }
    }

    println!();

    // Process the model
    let start = Instant::now();
    let stats = process_model(&input, &output, &config)?;
    let elapsed = start.elapsed();

    // Print results
    println!("{} Model processed!", style("✓").green().bold());
    println!("  Output: {}", style(output.display()).cyan());
    println!(
        "  Size: {} → {} ({:.1}%)",
        style(format_size(stats.original_size)).dim(),
        style(format_size(stats.output_size)).green(),
        if stats.original_size > 0 {
            (1.0 - stats.output_size as f64 / stats.original_size as f64) * 100.0
        } else {
            0.0
        }
    );
    println!("  Time: {:.2}s", elapsed.as_secs_f64());

    Ok(())
}

fn print_model_info(input: &PathBuf) -> Result<()> {
    let format = detect_model_format(input)
        .ok_or_else(|| anyhow::anyhow!("Unsupported model format"))?;

    let info = get_model_info(input)?;

    println!("{} Model Information", style("📊").blue().bold());
    println!("  File: {}", style(input.display()).cyan());
    println!("  Format: {}", style(format).cyan());
    println!();
    println!("  {}", style("Geometry:").bold());
    println!("    Meshes: {}", info.meshes);
    println!("    Vertices: {}", info.total_vertices);
    println!("    Indices: {}", info.total_indices);
    println!("    Triangles: ~{}", info.total_indices / 3);
    println!();
    println!("  {}", style("Resources:").bold());
    println!("    Materials: {}", info.materials);
    println!("    Textures: {}", info.textures);
    println!("    Animations: {}", info.animations);
    println!("    Nodes: {}", info.nodes);

    // Show LOD recommendations
    let lod_estimates = estimate_lod_levels(&info);
    if lod_estimates.len() > 1 {
        println!();
        println!("  {}", style("Recommended LOD Levels:").bold());
        for est in &lod_estimates {
            println!(
                "    LOD {}: {:.0}% vertices (~{} triangles) at distance {}",
                est.level,
                est.vertex_ratio * 100.0,
                est.estimated_triangles,
                est.suggested_distance
            );
        }
    }

    // File size
    let file_size = std::fs::metadata(input)?.len();
    println!();
    println!("  File size: {}", format_size(file_size));

    Ok(())
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
