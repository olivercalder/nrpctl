use crate::cli::Command;
use crate::config::Config;
use crate::snap;
use anyhow::{anyhow, Context, Result};
use std::ffi::OsString;
use std::fs;
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};

/// Run the given command with the given config file.
///
/// The functions called may print to stdout or otherwise provide user feedback.
pub fn run(cmd: Command, config_path: PathBuf) -> Result<()> {
    match cmd {
        Command::Init { sites_enabled_dir } => init(config_path, sites_enabled_dir),
        Command::Status => status(&config_path),
        Command::Render { listen_domain } => render(&config_path, listen_domain),
        Command::Add {
            listen_port,
            dest_domain,
            listen_domain,
            dest_port,
        } => add(
            &config_path,
            listen_domain,
            listen_port,
            dest_domain,
            dest_port,
        ),
        Command::Remove { listen_domain } => remove(&config_path, listen_domain),
        Command::Disable { listen_domain } => disable(&config_path, listen_domain),
        Command::Enable { listen_domain } => enable(&config_path, listen_domain),
    }
}

fn init(config_path: PathBuf, sites_enabled_dir: Option<PathBuf>) -> Result<()> {
    let nginx_dir = sites_enabled_dir.unwrap_or_else(|| {
        snap::nginx_sites_enabled_dir().unwrap_or(PathBuf::from("/etc/nginx/sites-enabled"))
    });
    let config = Config::new(nginx_dir.clone());
    if fs::exists(&config_path)
        .with_context(|| format!("Failed to check if config file exists: {config_path:?}"))?
    {
        return Err(anyhow!(
            "Failed to initialize config: file already exists: {config_path:?}"
        ));
    }
    fs::create_dir_all(&nginx_dir).context(format!(
        "Failed to create nginx sites enabled dir: {nginx_dir:?}",
    ))?;
    config.write(&config_path)?;
    println!("Successfully initialized nrpctl config at {config_path:?}");
    Ok(())
}

fn status(config_path: &Path) -> Result<()> {
    let config = Config::read(config_path)?;
    // TODO: make nice table instead of just pretty-printing toml
    let table = toml::to_string_pretty(&config)?;
    Ok(stdout().lock().write_all(table.as_bytes())?)
}

fn render(config_path: &Path, listen_domain: Option<String>) -> Result<()> {
    let config = Config::read(config_path)?;
    if let Some(domain) = listen_domain {
        let rendered = config.render(&domain)?;
        return Ok(stdout().lock().write_all(rendered.as_bytes())?);
    }

    let mut stdout = stdout().lock();
    for rendered in config.render_all() {
        stdout.write_all(rendered.as_bytes())?;
    }
    Ok(())
}

fn add(
    config_path: &Path,
    listen_domain: String,
    listen_port: u16,
    dest_domain: String,
    dest_port: u16,
) -> Result<()> {
    let mut config = Config::read(config_path)?;
    if config.add(listen_domain.clone(), listen_port, dest_domain, dest_port) {
        return Err(anyhow!(
            "Proxy already exists with the given domain: {listen_domain}",
        ));
    }

    config.write_nginx_site(&listen_domain)?;
    // TODO: delete written file if there's a later error

    // TODO: test the configuration properly using nginx -t
    backup_and_write_config(config_path, &config)?;
    println!("Successfully added {listen_domain}");
    Ok(())
}

fn remove(config_path: &Path, listen_domain: String) -> Result<()> {
    let mut config = Config::read(config_path)?;
    config.remove(&listen_domain); // treat remove as idempotent, don't error

    config.delete_nginx_site(&listen_domain)?;
    // TODO: restore written file if there's a later error

    // TODO: test the configuration properly using nginx -t
    backup_and_write_config(config_path, &config)?;
    println!("Successfully removed {listen_domain}");
    Ok(())
}

fn disable(config_path: &Path, listen_domain: String) -> Result<()> {
    let mut config = Config::read(config_path)?;
    if config.disable(&listen_domain).is_none() {
        return Err(anyhow!(
            "Cannot disable proxy which does not exist: {listen_domain}",
        ));
    };

    config.delete_nginx_site(&listen_domain)?;
    // TODO: restore written file if there's a later error

    // TODO: test the configuration properly using nginx -t
    backup_and_write_config(config_path, &config)?;
    println!("Successfully disabled {listen_domain}");
    Ok(())
}

fn enable(config_path: &Path, listen_domain: String) -> Result<()> {
    let mut config = Config::read(config_path)?;
    if config.enable(&listen_domain).is_none() {
        return Err(anyhow!(
            "Cannot enable proxy which does not exist: {listen_domain}"
        ));
    };

    config.write_nginx_site(&listen_domain)?;
    // TODO: delete written file if there's a later error

    // TODO: test the configuration properly using nginx -t
    backup_and_write_config(config_path, &config)?;
    println!("Successfully enabled {listen_domain}");
    Ok(())
}

fn backup_and_write_config(config_path: &Path, config: &Config) -> Result<()> {
    let backup_path = config_backup_path(config_path);
    fs::rename(config_path, backup_path)?;
    config.write(config_path)
}

fn config_backup_path(config_path: &Path) -> PathBuf {
    let ext = config_path.extension().map_or(OsString::from("bak"), |e| {
        let mut ext = e.to_os_string();
        ext.push(".bak");
        ext
    });
    config_path.with_extension(&ext)
}
