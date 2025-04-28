use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::proxy::{Proxy, ProxySetting, ProxySettingKey, ProxySettingKeyOptional};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    sites_enabled_dir: PathBuf,
    proxies: BTreeMap<String, Proxy>,
}

impl Config {
    /// Create a new nrpctl config.
    pub fn new(sites_enabled_dir: PathBuf) -> Config {
        let sites_enabled_dir = if sites_enabled_dir.is_relative() {
            env::current_dir() // only get cwd if we need to
                .expect("Failed to get current working directory")
                .join(sites_enabled_dir)
        } else {
            sites_enabled_dir
        };
        Config {
            sites_enabled_dir,
            proxies: BTreeMap::new(),
        }
    }

    /// Read and return an existing nrpctl config from the given path. Returns an error if the
    /// config does not exist or cannot be parsed.
    pub fn read(path: &Path) -> Result<Config> {
        let data =
            fs::read_to_string(path).context(format!("Failed to open config file {path:?}"))?;
        let config: Config = toml::from_str(data.as_str())
            .context(format!("Failed to parse config file {path:?}"))?;
        Ok(config)
    }

    /// Write the config to disk at the given path. Returns an error if the config cannot be
    /// serialized or the file cannot be written.
    pub fn write(&self, path: &Path) -> Result<()> {
        let data = toml::to_string_pretty(self).context("Failed to format config as toml")?;
        fs::write(path, data).context(format!("Failed to write config file: {path:?}"))?;
        Ok(())
    }

    /// Return the path for the nginx config file associated with the given domain.
    fn site_config_path(&self, listen_domain: &str) -> PathBuf {
        Path::new(&self.sites_enabled_dir).join(listen_domain)
    }

    /// Render and write the nginx site configuration for the given domain to the sites enabled
    /// directory. Returns a closure which will delete the new configuration and, if it existed,
    /// restore the previous configuration.
    pub fn write_nginx_site(
        &self,
        listen_domain: &str,
    ) -> Result<Box<dyn FnOnce() -> Result<String>>> {
        let path = self.site_config_path(listen_domain);
        let rendered = self.render(listen_domain)?;

        let cloned_domain = String::from(listen_domain);
        let cloned_path = path.clone();
        let restore: Box<dyn FnOnce() -> Result<String>> = match fs::read(&path) {
            Ok(contents) => Box::new(move || {
                fs::write(&cloned_path, contents).with_context(|| {
                    format!(
                        "Failed to restore prior nginx config file for {}: {:?}",
                        cloned_domain, cloned_path
                    )
                })?;
                Ok(format!(
                    "Restored prior nginx config file for {}: {:?}",
                    cloned_domain, cloned_path
                ))
            }),
            Err(_) => Box::new(move || {
                fs::remove_file(&cloned_path).with_context(|| {
                    format!(
                        "Failed to clean up nginx config file for {} after error caused rollback: {:?}", cloned_domain, cloned_path
                    )
                })?;
                Ok(format!(
                    "Cleaned up nginx config file for {} after error caused rollback: {:?}",
                    cloned_domain, cloned_path
                ))
            }),
        };

        fs::write(&path, rendered).with_context(|| {
            format!(
                "Failed to write nginx config file for {}: {:?}",
                listen_domain, path
            )
        })?;

        Ok(restore)
    }

    /// Delete the nginx site configuration for the given domain from the sites enabled directory.
    /// Returns a closure which will restore the previous configuration, if it existed.
    pub fn delete_nginx_site(
        &self,
        listen_domain: &str,
    ) -> Result<Box<dyn FnOnce() -> Result<String>>> {
        let path = self.site_config_path(listen_domain);

        let cloned_domain = String::from(listen_domain);
        let cloned_path = path.clone();
        let restore: Box<dyn FnOnce() -> Result<String>> = match fs::read(&path) {
            Ok(contents) => Box::new(move || {
                fs::write(&cloned_path, contents).with_context(|| {
                    format!(
                        "Failed to restore prior nginx config file for {}: {:?}",
                        cloned_domain, cloned_path
                    )
                })?;
                Ok(format!(
                    "Restored prior nginx config file for {}: {:?}",
                    cloned_domain, cloned_path
                ))
            }),
            Err(_) => Box::new(|| Ok(String::new())), // this will cause unfortunate empty line
        };

        fs::remove_file(&path).context(format!(
            "Failed to delete nginx config file for {listen_domain}: {path:?}"
        ))?;

        Ok(restore)
    }

    /// Render the nginx configuration for the given domain.
    pub fn render(&self, listen_domain: &str) -> Result<String> {
        let proxy = self.proxies.get(listen_domain).ok_or(anyhow!(
            "Failed to find proxy for the given domain: {listen_domain}"
        ))?;
        let rendered = proxy.render(listen_domain);
        Ok(rendered)
    }

    /// Render the nginx configurations for all proxies.
    pub fn render_all(&self) -> impl '_ + Iterator<Item = String> {
        self.proxies.iter().map(|(dom, prox)| prox.render(dom))
    }

    /// Add a new proxy with the given contents. If a proxy already existed with the same listen
    /// domain, replace it, and return true.
    pub fn add(
        &mut self,
        listen_domain: String,
        listen_port: u16,
        dest_domain: String,
        dest_port: u16,
        client_max_body_size: Option<String>,
        gzip: Option<bool>,
    ) -> bool {
        let proxy = Proxy::new(
            listen_port,
            dest_domain,
            dest_port,
            client_max_body_size,
            gzip,
        );
        self.proxies.insert(listen_domain, proxy).is_some()
    }

    /// Remove the proxy for the given domain, if it exists, and return true if so.
    pub fn remove(&mut self, listen_domain: &str) -> bool {
        self.proxies.remove(listen_domain).is_some()
    }

    /// Disable the given domain, if it exists, and return true if it was previously disabled.
    pub fn disable(&mut self, listen_domain: &str) -> Result<bool> {
        let proxy = self.proxies.get_mut(listen_domain).ok_or(anyhow!(
            "Failed to find proxy for the given domain: {listen_domain}"
        ))?;
        let prev = proxy.disable();
        // Most of the time, we use None to indicate "not disabled", so ensure that we return
        // Some(false) here if the proxy exists and was not disabled, to differentiate it from the
        // case where the proxy doesn't exist at all.
        Ok(prev)
    }

    /// Enable the given domain, if it exists, and return true if it was previously disabled.
    /// If a proxy does not exist for the given domain, returns `None`.
    pub fn enable(&mut self, listen_domain: &str) -> Result<bool> {
        let proxy = self.proxies.get_mut(listen_domain).ok_or(anyhow!(
            "Failed to find proxy for the given domain: {listen_domain}"
        ))?;
        let prev = proxy.enable();
        // Most of the time, we use None to indicate "not disabled", so ensure that we return
        // Some(false) here if the proxy exists and was not disabled, to differentiate it from the
        // case where the proxy doesn't exist at all.
        Ok(prev)
    }

    /// Get the value of the given proxy setting for the given domain.
    pub fn get_key(&self, listen_domain: &str, key: ProxySettingKey) -> Result<ProxySetting> {
        let proxy = self.proxies.get(listen_domain).ok_or(anyhow!(
            "Failed to find proxy for the given domain: {listen_domain}"
        ))?;
        Ok(proxy.get_key(key))
    }

    /// Set the value of the given proxy setting for the given domain to the given value, and
    /// return it.
    pub fn set_key(
        &mut self,
        listen_domain: &str,
        key: ProxySettingKey,
        value: String,
    ) -> Result<ProxySetting> {
        let proxy = self.proxies.get_mut(listen_domain).ok_or(anyhow!(
            "Failed to find proxy for the given domain: {listen_domain}"
        ))?;
        proxy.set_key(key, value)
    }

    /// Unset the value of the given proxy setting for the given domain and return the previous
    /// setting.
    pub fn unset_key(
        &mut self,
        listen_domain: &str,
        key: ProxySettingKeyOptional,
    ) -> Result<ProxySetting> {
        let proxy = self.proxies.get_mut(listen_domain).ok_or(anyhow!(
            "Failed to find proxy for the given domain: {listen_domain}"
        ))?;
        Ok(proxy.unset_key(key))
    }
}
