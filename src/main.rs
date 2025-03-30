use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod cli;
mod client;
mod config;
mod snap;
// TODO: mod certbot;
// TODO: mod nginx;

use crate::cli::Cli;

fn main() -> Result<()> {
    let args = Cli::parse();

    let path = args
        .config
        .unwrap_or_else(|| snap::config_path().unwrap_or(PathBuf::from("/etc/nrpctl/config.toml")));

    client::run(args.command, path)
}
