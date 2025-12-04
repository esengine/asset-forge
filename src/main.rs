mod cli;
mod commands;
mod config;
mod processors;
mod utils;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use cli::{Cli, Commands};

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "asset_forge=info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer().without_time())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { force } => commands::init::run(force),
        Commands::Optimize { input, options } => commands::optimize::run(input, options),
        Commands::Build { input, options } => commands::build::run(input, options),
        Commands::Atlas { input, options } => commands::atlas::run(input, options),
        Commands::Watch { input, options } => commands::watch::run(input, options),
        Commands::Model { input, options } => commands::model::run(input, options),
    }
}
