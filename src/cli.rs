use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::{ProxySettingKey, ProxySettingKeyOptional};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Specify the path to the nrpctl configuration TOML file
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new nrpctl configuration file
    Init {
        /// The directory in which to write nginx configurations
        #[arg(short, long, value_name = "DIR")]
        sites_enabled_dir: Option<PathBuf>,
    },

    /// Display information about active reverse proxies
    Status,

    /// Render nginx configurations for reverse proxies
    Render {
        /// Render only the configuration for the given domain
        listen_domain: Option<String>,
    },

    /// Add a new reverse proxy
    Add {
        /// The port on which to listen for requests
        #[arg(short, long, default_value_t = 80)]
        listen_port: u16,

        /// The destination domain to which to forward requests
        #[arg(long, default_value_t = String::from("localhost"))]
        dest_domain: String,

        /// The source domain for which to listen (e.g. `cloud.mydomain.com`)
        listen_domain: String,

        /// The destination port to which to forward matching requests
        dest_port: u16,
        // TODO:
        // /// Set up SSL encryption (HTTPS) using certbot (requires listen port to be 80)
        // #[arg(short, long)]
        // ssl: bool,
        // TODO:
        // /// Listen for www.<SOURCE_DOMAIN> as well
        // #[arg(short, long)]
        // www: bool,
    },

    /// Remove a reverse proxy
    Remove {
        /// The source domain for this reverse proxy
        listen_domain: String,
    },

    /// Disable a reverse proxy without fully removing it
    Disable {
        /// The source domain for this reverse proxy
        listen_domain: String,
    },

    /// Enable a reverse proxy which was previously disabled
    Enable {
        /// The source domain for this reverse proxy
        listen_domain: String,
    },

    /// Get a configuration value for a reverse proxy
    Get {
        /// The source domain for this reverse proxy
        listen_domain: String,

        /// The configuration to get (gets all if unspecified)
        key: Option<ProxySettingKey>,
    },

    /// Set a configuration value for a reverse proxy
    Set {
        /// The source domain for this reverse proxy
        listen_domain: String,

        /// The configuration key to set
        key: ProxySettingKey,

        /// The value to set for the configuration key
        value: String,
    },

    /// Unset a configuration value for a reverse proxy
    Unset {
        /// The source domain for this reverse proxy
        listen_domain: String,

        /// The configuration key to unset
        key: ProxySettingKeyOptional,
    },
}
