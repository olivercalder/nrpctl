use anyhow::Result;
use clap::Parser;

mod certbot;
mod cli;
mod client;
mod config;
mod nginx;
mod proxy;
mod snap;
mod transaction;

use crate::cli::Cli;

fn main() -> Result<()> {
    let args = Cli::parse();

    client::run(args.command, args.config)
}
