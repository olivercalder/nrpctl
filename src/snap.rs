use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};

/// Returns the path the nrpctl config if it is running in a snap.
pub fn config_path() -> Result<PathBuf> {
    Ok(Path::new(&env::var("SNAP_COMMON").context("Not running in a snap")?).join("config.toml"))
}

/// Returns the path the nginx sites enabled directory if running in a snap.
pub fn nginx_sites_enabled_dir() -> Result<PathBuf> {
    Ok(
        Path::new(&env::var("SNAP_COMMON").context("Not running in a snap")?)
            .join("nginx-sites-enabled"),
    )
}
