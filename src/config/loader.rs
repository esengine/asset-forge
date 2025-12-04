use anyhow::{Context, Result};
use std::path::Path;

use super::Config;

/// Load configuration from a TOML file
pub fn load_config(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

    Ok(config)
}

/// Find and load configuration file
/// Searches in current directory and parent directories for asset-forge.toml
pub fn find_and_load_config() -> Result<Option<Config>> {
    let config_names = ["asset-forge.toml", ".asset-forge.toml"];

    let mut current_dir = std::env::current_dir()?;

    loop {
        for name in &config_names {
            let config_path = current_dir.join(name);
            if config_path.exists() {
                let config = load_config(&config_path)?;
                return Ok(Some(config));
            }
        }

        if !current_dir.pop() {
            break;
        }
    }

    Ok(None)
}

/// Save configuration to a TOML file
pub fn save_config(config: &Config, path: &Path) -> Result<()> {
    let content = toml::to_string_pretty(config)
        .context("Failed to serialize config")?;

    std::fs::write(path, content)
        .with_context(|| format!("Failed to write config file: {}", path.display()))?;

    Ok(())
}
