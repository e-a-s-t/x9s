mod app;
mod crossplane;
mod kubernetes;
mod model;
mod ui;

use std::{
    fs::{self, OpenOptions},
    io,
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "x9s",
    version = env!("CARGO_PKG_VERSION"),
    about = "Read-only Crossplane resource explorer"
)]
struct Args {}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = Args::parse();
    init_tracing()?;

    ui::run().await
}

fn init_tracing() -> Result<()> {
    let log_path = std::env::var_os("X9S_LOG_FILE").map(PathBuf::from);
    let diagnostics_enabled = log_path.is_some();
    let default_filter = if diagnostics_enabled { "info" } else { "warn" };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    if let Some(path) = log_path.as_ref() {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Could not create diagnostics directory {}",
                    parent.display()
                )
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("Could not open diagnostics file {}", path.display()))?;
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(file)
            .try_init()
            .map_err(|error| anyhow::anyhow!("Could not initialize diagnostic logging: {error}"))?;
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(io::sink)
            .try_init()
            .map_err(|error| anyhow::anyhow!("Could not initialize diagnostic logging: {error}"))?;
    }
    Ok(())
}
