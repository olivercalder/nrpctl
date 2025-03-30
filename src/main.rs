use anyhow::Result;
use clap::Parser;
use std::env;
use std::path::{Path, PathBuf};

// TODO: mod certbot;
mod cli;
mod config;
mod manager;

use crate::cli::Cli;

fn main() -> Result<()> {
    let args = Cli::parse();

    let path = if let Some(path) = args.config {
        path
    } else if let Ok(val) = env::var("SNAP_COMMON") {
        // We're running in a snap
        Path::new(&val).join("config.toml")
    } else {
        PathBuf::from("/etc/nrpctl/config.toml")
    };

    manager::run(args.command, path)
}
