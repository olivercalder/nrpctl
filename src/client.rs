use anyhow::{bail, Context, Result};
use std::ffi::OsString;
use std::fs;
use std::io::{stdout, Write};
use std::path::{Path, PathBuf};
use strum::IntoEnumIterator;

use crate::certbot::{self, SSLSelection};
use crate::cli::Command;
use crate::config::Config;
use crate::nginx;
use crate::proxy::{ProxySetting, ProxySettingKey, ProxySettingKeyOptional};
use crate::snap;
use crate::transaction::Transaction;

/// Run the given command with the given config file.
///
/// The functions called may print to stdout or otherwise provide user feedback.
pub fn run(cmd: Command, config_path: String) -> Result<()> {
    // Ignore config path if we're running in a snap
    let config_path = snap::config_path().unwrap_or(config_path.into());

    match cmd {
        Command::Init { sites_enabled_dir } => init(config_path, sites_enabled_dir.into()),
        Command::Status => status(&config_path),
        Command::Render { listen_domain } => render(&config_path, listen_domain),
        Command::Add {
            listen_port,
            dest_domain,
            listen_domain,
            dest_port,
            client_max_body_size,
            gzip,
            ipv6,
            ssl,
        } => add(
            &config_path,
            listen_domain,
            listen_port,
            dest_domain,
            dest_port,
            client_max_body_size,
            gzip,
            ipv6,
            ssl,
        ),
        Command::Remove { listen_domain } => remove(&config_path, listen_domain),
        Command::Disable { listen_domain } => disable(&config_path, listen_domain),
        Command::Enable { listen_domain } => enable(&config_path, listen_domain),
        Command::Get { listen_domain, key } => get(&config_path, listen_domain, key),
        Command::Set {
            listen_domain,
            key,
            value,
        } => set(&config_path, listen_domain, key, value),
        Command::Unset { listen_domain, key } => unset(&config_path, listen_domain, key),
    }
}

fn init(config_path: PathBuf, sites_enabled_dir: PathBuf) -> Result<()> {
    // If we're in a snap, ignore the given sites_enabled_dir
    let sites_enabled_dir = snap::nginx_sites_enabled_dir().unwrap_or(sites_enabled_dir);
    let config = Config::new(sites_enabled_dir.clone());
    if fs::exists(&config_path)
        .with_context(|| format!("Failed to check if config file exists: {config_path:?}"))?
    {
        bail!("Failed to initialize config: file already exists: {config_path:?}");
    }
    fs::create_dir_all(&sites_enabled_dir).with_context(|| {
        format!("Failed to create nginx sites enabled dir: {sites_enabled_dir:?}")
    })?;
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
    mut listen_port: u16,
    dest_domain: String,
    dest_port: u16,
    client_max_body_size: Option<String>,
    gzip: bool,
    ipv6: bool,
    ssl: Option<SSLSelection>,
) -> Result<()> {
    // Convert gzip bool to Option<bool>
    let gzip = if gzip { Some(true) } else { None };
    let ipv6 = if ipv6 { Some(true) } else { None };

    // Ensure that if SSL is set to redirect from port 80, then the listen port is not also port 80
    if Some(SSLSelection::Redirect) == ssl && listen_port == 80 {
        listen_port = 443;
    }

    let mut config = Config::read(config_path)?;
    if config.add(
        listen_domain.clone(),
        listen_port,
        dest_domain,
        dest_port,
        client_max_body_size,
        gzip,
        ipv6,
        ssl,
    ) {
        bail!("Proxy already exists with the given domain: {listen_domain}");
    }

    let mut transaction = Transaction::new();

    transaction.do_or_rollback(|| config.write_nginx_site(&listen_domain))?;

    transaction.do_or_rollback(nginx::reload)?;

    let ssl_selection = match ssl {
        Some(sel) => sel,
        None => SSLSelection::False,
    };

    transaction.do_or_rollback(|| certbot::handle_ssl_selection(&listen_domain, ssl_selection))?;

    transaction.do_or_rollback(|| backup_and_write_config(config_path, &config))?;

    println!("Successfully added {listen_domain}");
    Ok(())
}

fn remove(config_path: &Path, listen_domain: String) -> Result<()> {
    let mut config = Config::read(config_path)?;
    config.remove(&listen_domain); // treat remove as idempotent, don't error

    let mut transaction = Transaction::new();

    transaction.do_or_rollback(|| config.delete_nginx_site(&listen_domain))?;

    transaction.do_or_rollback(nginx::reload)?;

    transaction.do_or_rollback(|| backup_and_write_config(config_path, &config))?;

    println!("Successfully removed {listen_domain}");
    Ok(())
}

fn disable(config_path: &Path, listen_domain: String) -> Result<()> {
    let mut config = Config::read(config_path)?;
    config.disable(&listen_domain)?;

    let mut transaction = Transaction::new();

    transaction.do_or_rollback(|| config.delete_nginx_site(&listen_domain))?;

    transaction.do_or_rollback(nginx::reload)?;

    transaction.do_or_rollback(|| backup_and_write_config(config_path, &config))?;

    println!("Successfully disabled {listen_domain}");
    Ok(())
}

fn enable(config_path: &Path, listen_domain: String) -> Result<()> {
    let mut config = Config::read(config_path)?;
    config.enable(&listen_domain)?;

    let mut transaction = Transaction::new();

    transaction.do_or_rollback(|| config.write_nginx_site(&listen_domain))?;

    transaction.do_or_rollback(nginx::reload)?;

    transaction.do_or_rollback(|| backup_and_write_config(config_path, &config))?;

    println!("Successfully enabled {listen_domain}");
    Ok(())
}

fn get(config_path: &Path, listen_domain: String, key: Option<ProxySettingKey>) -> Result<()> {
    let config = Config::read(config_path)?;
    if let Some(k) = key {
        let setting = config.get_key(&listen_domain, k)?;
        println!("{}", &setting);
        return Ok(());
    }
    // No key was specified, so display all settings
    for k in ProxySettingKey::iter() {
        let setting = config.get_key(&listen_domain, k)?;
        println!("{}", &setting);
    }
    Ok(())
}

fn set(
    config_path: &Path,
    listen_domain: String,
    key: ProxySettingKey,
    value: String,
) -> Result<()> {
    let mut config = Config::read(config_path)?;
    let prev = config.get_key(&listen_domain, key)?;
    let setting = config.set_key(&listen_domain, key, value)?;
    // If we're changing the SSL setting to redirect, make sure listen-port is changed from 80
    let mut redirected_port = false;
    if let ProxySetting::Ssl(Some(SSLSelection::Redirect)) = setting {
        let prev_port = config.get_key(&listen_domain, ProxySettingKey::ListenPort)?; // should not fail
        if let ProxySetting::ListenPort(p) = prev_port {
            if p == 80 {
                redirected_port = true;
                config.set_key(
                    &listen_domain,
                    ProxySettingKey::ListenPort,
                    String::from("443"),
                )?; // should not fail
            }
        }
    }

    let mut transaction = Transaction::new();

    transaction.do_or_rollback(|| config.write_nginx_site(&listen_domain))?;

    transaction.do_or_rollback(nginx::reload)?;

    if let ProxySetting::Ssl(Some(sel)) = &setting {
        match prev {
            ProxySetting::Ssl(None) | ProxySetting::Ssl(Some(SSLSelection::False)) => {
                transaction
                    .do_or_rollback(|| certbot::handle_ssl_selection(&listen_domain, *sel))?;
            }
            _ => {} // only get SSL cert if there was previously no SSL and now there is
        }
    }

    transaction.do_or_rollback(|| backup_and_write_config(config_path, &config))?;

    println!("Successfully set: {}", &setting);
    if redirected_port {
        println!(
            "Successfully changed \"listen-port\" from port 80 to port 443 due to SSL redirection"
        )
    }
    Ok(())
}

fn unset(config_path: &Path, listen_domain: String, key: ProxySettingKeyOptional) -> Result<()> {
    let mut config = Config::read(config_path)?;
    let setting = config.unset_key(&listen_domain, key)?;

    let mut transaction = Transaction::new();

    transaction.do_or_rollback(|| config.write_nginx_site(&listen_domain))?;

    transaction.do_or_rollback(nginx::reload)?;

    transaction.do_or_rollback(|| backup_and_write_config(config_path, &config))?;

    println!("Successfully unset {key}; Previous value: {}", &setting);
    Ok(())
}

fn backup_and_write_config(
    config_path: &Path,
    config: &Config,
) -> Result<Box<dyn FnOnce() -> Result<String>>> {
    let backup_path = config_backup_path(config_path);
    fs::rename(config_path, backup_path)?;
    let cloned_path = config_path.to_path_buf();
    let restore: Box<dyn FnOnce() -> Result<String>> = match fs::read(config_path) {
        Ok(contents) => Box::new(move || {
            fs::write(&cloned_path, contents).with_context(|| {
                format!("Failed to restore prior nrpctl configuration file at {cloned_path:?}",)
            })?;
            Ok(format!(
                "Restored prior nrpctl configuration file at {cloned_path:?}",
            ))
        }),
        Err(_) => Box::new(move || {
            fs::remove_file(&cloned_path).with_context(|| {
                format!(
                    "Failed to clean up nrpctl configuration file after error caused rollback: {cloned_path:?}"
                )
            })?;
            Ok(format!(
                "Cleaned up nrpctl configuration file after error caused rollback: {cloned_path:?}"
            ))
        }),
    };
    config.write(config_path)?;

    Ok(restore)
}

fn config_backup_path(config_path: &Path) -> PathBuf {
    let ext = config_path.extension().map_or(OsString::from("bak"), |e| {
        let mut ext = e.to_os_string();
        ext.push(".bak");
        ext
    });
    config_path.with_extension(&ext)
}
