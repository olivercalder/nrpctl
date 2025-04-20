use anyhow::Result;
use clap::Parser;

mod cli;
mod client;
mod config;
mod snap;
// TODO: mod certbot;
// TODO: mod nginx;

use crate::cli::Cli;

fn main() -> Result<()> {
    let args = Cli::parse();

    client::run(args.command, args.config)
}
