use anyhow::Result;
use console::style;
use std::path::PathBuf;
use std::time::Instant;

use crate::cli::{AudioOptions, AudioOutputFormat};
use crate::processors::{process_audio, get_audio_info, AudioConfig, AudioFormat};

pub fn run(input: PathBuf, options: AudioOptions) -> Result<()> {
    if !input.exists() {
        anyhow::bail!("Input file does not exist: {}", input.display());
    }

    // Info-only mode
    if options.info {
        return print_audio_info(&input);
    }

    // Determine output path
    let output_format = match options.format {
        AudioOutputFormat::Ogg => AudioFormat::Ogg,
        AudioOutputFormat::Wav => AudioFormat::Wav,
    };

    let output = options.output.unwrap_or_else(|| {
        let stem = input.file_stem().unwrap_or_default();
        let ext = match output_format {
            AudioFormat::Ogg => "ogg",
            AudioFormat::Wav => "wav",
        };
        let default_dir = PathBuf::from(".");
        let parent = input.parent().unwrap_or(&default_dir);
        parent.join(format!("{}.{}", stem.to_string_lossy(), ext))
    });

    println!(
        "{} Processing audio: {}",
        style("→").blue().bold(),
        input.display()
    );

    // Get and display audio info
    let info = get_audio_info(&input)?;
    println!("  Channels: {}", style(info.channels).cyan());
    println!("  Sample rate: {} Hz", style(info.sample_rate).cyan());
    println!("  Duration: {:.2}s", style(info.duration_secs).cyan());
    println!("  Format: {}", style(&info.format).cyan());

    // Build config
    let config = AudioConfig {
        output_format,
        quality: options.quality as f32 / 10.0, // Convert 1-10 to 0.1-1.0
        sample_rate: options.sample_rate,
        normalize: options.normalize,
    };

    // Show processing options
    println!();
    println!("{} Processing options:", style("⚙").blue().bold());
    println!("  Output format: {}", style(options.format).cyan());
    if output_format == AudioFormat::Ogg {
        println!("  Quality: {}/10", style(options.quality).cyan());
    }
    if let Some(rate) = options.sample_rate {
        println!("  Target sample rate: {} Hz", style(rate).cyan());
    }
    if options.normalize {
        println!("  {} Normalize volume", style("✓").green());
    }
    println!();

    // Process the audio
    let start = Instant::now();
    let stats = process_audio(&input, &output, &config)?;
    let elapsed = start.elapsed();

    // Print results
    println!("{} Audio processed!", style("✓").green().bold());
    println!("  Output: {}", style(output.display()).cyan());
    println!(
        "  Size: {} → {} ({})",
        format_size(stats.original_size),
        style(format_size(stats.output_size)).green(),
        format_reduction(stats.original_size, stats.output_size)
    );
    println!("  Time: {:.2}s", elapsed.as_secs_f64());

    Ok(())
}

fn print_audio_info(input: &PathBuf) -> Result<()> {
    let info = get_audio_info(input)?;
    let file_size = std::fs::metadata(input)?.len();

    println!("{} Audio Information", style("🔊").blue().bold());
    println!("  File: {}", style(input.display()).cyan());
    println!("  Format: {}", style(&info.format).cyan());
    println!();
    println!("  {}", style("Properties:").bold());
    println!("    Channels: {}", info.channels);
    println!("    Sample rate: {} Hz", info.sample_rate);
    println!("    Duration: {:.2}s", info.duration_secs);
    println!();
    println!("  File size: {}", format_size(file_size));

    // Bitrate estimate
    if info.duration_secs > 0.0 {
        let bitrate = (file_size as f64 * 8.0) / info.duration_secs / 1000.0;
        println!("  Bitrate: ~{:.0} kbps", bitrate);
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

fn format_reduction(original: u64, output: u64) -> String {
    if original == 0 {
        return "N/A".to_string();
    }

    let reduction = (1.0 - output as f64 / original as f64) * 100.0;
    if reduction > 0.0 {
        format!("{:.1}% smaller", reduction)
    } else if reduction < 0.0 {
        format!("{:.1}% larger", -reduction)
    } else {
        "same size".to_string()
    }
}
