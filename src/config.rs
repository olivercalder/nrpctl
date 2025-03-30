use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct Config {
    sites_enabled_dir: PathBuf,
    proxies: BTreeMap<String, Proxy>,
}

impl Config {
    /// Create a new nrpctl config.
    pub fn new(sites_enabled_dir: PathBuf) -> Config {
        let sites_enabled_dir = match sites_enabled_dir.is_relative() {
            true => env::current_dir() // only get cwd if we need to
                .expect("Failed to get current working directory")
                .join(sites_enabled_dir),
            false => sites_enabled_dir,
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

    /// Render and write the nginx site configuration for the given domain to the sites enabled
    /// directory.
    pub fn write_nginx_site(&self, listen_domain: &str) -> Result<()> {
        let path = Path::new(&self.sites_enabled_dir).join(listen_domain);
        let rendered = self.render(listen_domain)?;
        fs::write(&path, rendered).context(format!(
            "Failed to write nginx config file for {listen_domain}: {path:?}"
        ))
    }

    /// Delete the nginx site configuration for the given domain from the sites enabled directory.
    pub fn delete_nginx_site(&self, listen_domain: &str) -> Result<()> {
        let path = Path::new(&self.sites_enabled_dir).join(listen_domain);
        fs::remove_file(&path).context(format!(
            "Failed to delete nginx config file for {listen_domain}: {path:?}"
        ))
    }

    /// Render the nginx configuration for the given domain.
    pub fn render(&self, listen_domain: &str) -> Result<String> {
        let Some(proxy) = self.proxies.get(listen_domain) else {
            return Err(anyhow!(
                "Failed to find proxy for the given domain: {}",
                listen_domain
            ));
        };
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
    ) -> bool {
        let proxy = Proxy {
            listen_port,
            dest_domain,
            dest_port,
            client_max_body_size: None,
            disabled: None,
        };
        self.proxies.insert(listen_domain, proxy).is_some()
    }

    /// Remove the proxy for the given domain, if it exists, and return true if so.
    pub fn remove(&mut self, listen_domain: &str) -> bool {
        self.proxies.remove(listen_domain).is_some()
    }

    /// Disable the given domain, if it exists, and return true if it was previously disabled.
    /// If a proxy does not exist for the given domain, returns `None`.
    pub fn disable(&mut self, listen_domain: &str) -> Option<bool> {
        let proxy = self.proxies.get_mut(listen_domain)?;
        // Most of the time, we use None to indicate "not disabled", so ensure that we return
        // Some(false) here if the proxy exists and was not disabled, to differentiate it from the
        // case where the proxy doesn't exist at all.
        let current = matches!(proxy.disabled, Some(true));
        proxy.disabled = Some(true);
        Some(current)
    }

    /// Enable the given domain, if it exists, and return true if it was previously disabled.
    /// If a proxy does not exist for the given domain, returns `None`.
    pub fn enable(&mut self, listen_domain: &str) -> Option<bool> {
        let proxy = self.proxies.get_mut(listen_domain)?;
        // Most of the time, we use None to indicate "not disabled", so ensure that we return
        // Some(false) here if the proxy exists and was not disabled, to differentiate it from the
        // case where the proxy doesn't exist at all.
        let current = matches!(proxy.disabled, Some(true));
        proxy.disabled = None;
        Some(current)
    }
}

// Don't include listen_domain in the configuration, since that value is not configurable after the
// proxy has been created.
/// The configuration for a given reverse proxy
#[derive(Serialize, Deserialize)]
struct Proxy {
    /// The port on which to listen for matching requests
    listen_port: u16,

    /// The domain to which to forward requests
    dest_domain: String,

    /// The port to which to forward requests
    dest_port: u16,

    /// The maximum allowed size of the client request body (default: "1m")
    client_max_body_size: Option<String>,

    /// The proxy is disabled if set to true
    disabled: Option<bool>, // use None instead of Some(false) so false values omitted
}

impl Proxy {
    pub fn render(&self, listen_domain: &str) -> String {
        if self.disabled == Some(true) {
            return format!("\n# {} disabled\n", listen_domain);
        }

        let max_body_size = match &self.client_max_body_size {
            Some(s) => s,
            None => "",
        };

        format!(
            "
server {{
    server_name {};

    listen {};

    location / {{
        proxy_pass http://{}:{};

        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_redirect off;
    }}

    fastcgi_request_buffering off;
    {}
}}
",
            listen_domain, self.listen_port, self.dest_domain, self.dest_port, max_body_size
        )
    }
}
