use anyhow::{Context, Result};
use console::style;
use std::path::Path;

use crate::config::Config;

const CONFIG_FILE_NAME: &str = "asset-forge.toml";

pub fn run(force: bool) -> Result<()> {
    let config_path = Path::new(CONFIG_FILE_NAME);

    if config_path.exists() && !force {
        println!(
            "{} Configuration file already exists: {}",
            style("!").yellow().bold(),
            config_path.display()
        );
        println!("  Use {} to overwrite.", style("--force").cyan());
        return Ok(());
    }

    // Generate default configuration
    let content = Config::default_toml();

    std::fs::write(config_path, &content)
        .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

    println!(
        "{} Created configuration file: {}",
        style("✓").green().bold(),
        style(config_path.display()).cyan()
    );

    println!();
    println!("Next steps:");
    println!(
        "  1. Edit {} to configure your project",
        style(CONFIG_FILE_NAME).cyan()
    );
    println!(
        "  2. Run {} to process your assets",
        style("asset-forge build ./assets").cyan()
    );
    println!(
        "  3. Run {} for help",
        style("asset-forge --help").cyan()
    );

    Ok(())
}
