use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

// Don't include listen_domain in the configuration, since that value is not configurable after the
// proxy has been created.
/// The configuration for a given reverse proxy
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Proxy {
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

    /// If true, turns gzip on in the proxy configuration
    gzip: Option<bool>, // use None instead of Some(false) so false values omitted
}

impl Proxy {
    /// Create a new proxy with the given source and destination.
    pub fn new(listen_port: u16, dest_domain: String, dest_port: u16, gzip: Option<bool>) -> Proxy {
        Proxy {
            listen_port,
            dest_domain,
            dest_port,
            client_max_body_size: None,
            disabled: None,
            gzip,
        }
    }

    /// Disable the proxy. Returns whether the proxy was previously disabled.
    pub fn disable(&mut self) -> bool {
        let prev = matches!(self.disabled, Some(true));
        self.disabled = Some(true);
        prev
    }

    /// Enable the proxy. Returns whether the proxy was previously disabled.
    pub fn enable(&mut self) -> bool {
        let prev = matches!(self.disabled, Some(true));
        self.disabled = None;
        prev
    }

    /// Get the value of the given key.
    pub fn get_key(&self, key: ProxySettingKey) -> ProxySetting {
        match key {
            ProxySettingKey::ListenPort => ProxySetting::ListenPort(self.listen_port),
            ProxySettingKey::DestDomain => ProxySetting::DestDomain(self.dest_domain.clone()),
            ProxySettingKey::DestPort => ProxySetting::DestPort(self.dest_port),
            ProxySettingKey::ClientMaxBodySize => {
                ProxySetting::ClientMaxBodySize(self.client_max_body_size.clone())
            }
            ProxySettingKey::Disabled => ProxySetting::Disabled(self.disabled),
            ProxySettingKey::Gzip => ProxySetting::Gzip(self.gzip),
        }
    }

    /// Set the given key to the given value and return the new setting.
    pub fn set_key(&mut self, key: ProxySettingKey, value: String) -> Result<ProxySetting> {
        let setting = key.parse(value)?;
        match &setting {
            ProxySetting::ListenPort(port) => self.listen_port = *port,
            ProxySetting::DestDomain(domain) => self.dest_domain = domain.clone(),
            ProxySetting::DestPort(port) => self.dest_port = *port,
            ProxySetting::ClientMaxBodySize(size) => self.client_max_body_size = size.clone(),
            ProxySetting::Disabled(val) => self.disabled = *val,
            ProxySetting::Gzip(val) => self.gzip = *val,
        };
        Ok(setting)
    }

    /// Unset the given key and return its previous setting.
    pub fn unset_key(&mut self, key: ProxySettingKeyOptional) -> ProxySetting {
        match key {
            ProxySettingKeyOptional::ClientMaxBodySize => {
                let orig = self.client_max_body_size.clone();
                // TODO: would be nice to move the string instead of cloning, since we're about to replace it
                self.client_max_body_size = None;
                ProxySetting::ClientMaxBodySize(orig)
            }
            ProxySettingKeyOptional::Disabled => {
                let orig = self.disabled;
                self.disabled = None;
                ProxySetting::Disabled(orig)
            }
            ProxySettingKeyOptional::Gzip => {
                let orig = self.gzip;
                self.gzip = None;
                ProxySetting::Gzip(orig)
            }
        }
    }

    pub fn render(&self, listen_domain: &str) -> String {
        if self.disabled == Some(true) {
            return format!("\n# {} disabled\n", listen_domain);
        }

        let listen_port = self.listen_port;

        let gzip = if self.gzip == Some(true) {
            "
    gzip on;
    gzip_types
      application/javascript
      application/x-javascript
      application/json
      application/xml
      application/xml+rss
      application/rss+xml
      image/svg+xml
      image/xml+svg
      image/x-icon
      application/vnd.ms-fontobject
      application/font-sfnt
      text/css
      text/javascript
      text/plain
      text/xml;
    gzip_min_length 256;
    gzip_comp_level 5;
    gzip_http_version 1.1;
    gzip_proxy any;
    gzip_vary on;
"
        } else {
            ""
        };

        let dest_domain = &self.dest_domain;
        let dest_port = self.dest_port;

        let max_body_size = match &self.client_max_body_size {
            Some(s) => s,
            None => "",
        };

        format!(
            "# Managed by nrpctl -- any changes will be overwritten, so do not manually edit
server {{
    server_name {listen_domain};

    listen {listen_port};
{gzip}
    location / {{
        proxy_pass http://{dest_domain}:{dest_port};

        proxy_set_header Host $http_host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_redirect off;
    }}

    fastcgi_request_buffering off;
    {max_body_size}
}}
"
        )
    }
}

/// ProxySetting defines the keys and corresponding values which can be retrieved or set for a
/// given proxy.
pub enum ProxySetting {
    ListenPort(u16),
    DestDomain(String),
    DestPort(u16),
    ClientMaxBodySize(Option<String>),
    Disabled(Option<bool>),
    Gzip(Option<bool>),
}

impl fmt::Display for ProxySetting {
    // Implement display which is compatible with toml for each proxy setting.
    // For `None` variants, return a toml comment saying "# <setting> is unset".
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProxySetting::ListenPort(port) => {
                write!(f, "\"listen-port\" = {}", port)
            }
            ProxySetting::DestDomain(ref s) => {
                write!(f, "\"dest-domain\" = \"{}\"", s)
            }
            ProxySetting::DestPort(port) => {
                write!(f, "\"dest-port\" = {}", port)
            }
            ProxySetting::ClientMaxBodySize(ref maybe) => match maybe {
                Some(size) => write!(f, "\"client-max-body-size\" = {}", size),
                None => write!(f, "# \"client-max-body-size\" is unset"),
            },
            ProxySetting::Disabled(ref maybe) => match maybe {
                Some(val) => write!(f, "\"disabled\" = {}", val),
                None => write!(f, "# \"disabled\" is unset"),
            },
            ProxySetting::Gzip(ref maybe) => match maybe {
                Some(val) => write!(f, "\"gzip\" = {}", val),
                None => write!(f, "# \"gzip\" is unset"),
            },
        }
    }
}

/// ProxySettingKeyOptional defines the keys which are optional and can be unset for a given proxy.
#[derive(clap::ValueEnum, Clone, Copy, Serialize)]
#[serde(tag = "key", rename_all = "kebab-case")]
pub enum ProxySettingKeyOptional {
    /// The maximum acceptable request body size (e.g. "512m")
    ClientMaxBodySize,
    /// Whether the reverse proxy is disabled
    Disabled,
    /// Whether to gzip response content
    Gzip,
}

impl fmt::Display for ProxySettingKeyOptional {
    // TODO: add thorough tests.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            ProxySettingKeyOptional::ClientMaxBodySize => "client-max-body-size",
            ProxySettingKeyOptional::Disabled => "disabled",
            ProxySettingKeyOptional::Gzip => "gzip",
        };
        write!(f, "{}", name)
    }
}

/// ProxySettingKey defines the keys which can be retrieved or set for a given proxy.
#[derive(clap::ValueEnum, strum::EnumIter, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxySettingKey {
    /// The port on which to listen for requests
    ListenPort,
    /// The destination domain to which to forward requests
    DestDomain,
    /// The destination port to which to forward requests
    DestPort,
    /// The maximum acceptable request body size (e.g. "512m")
    ClientMaxBodySize,
    /// Whether the reverse proxy is disabled
    Disabled,
    /// Whether to gzip response content
    Gzip,
}

impl ProxySettingKey {
    pub fn parse(&self, value: String) -> Result<ProxySetting> {
        Ok(match self {
            ProxySettingKey::ListenPort => {
                let port: u16 = value
                    .parse()
                    .context("Failed to parse value as listen port: {value}")?;
                ProxySetting::ListenPort(port)
            }
            ProxySettingKey::DestDomain => ProxySetting::DestDomain(value), // TODO: validate in some way
            ProxySettingKey::DestPort => {
                let port: u16 = value
                    .parse()
                    .context("Failed to parse value as destination port: {value}")?;
                ProxySetting::DestPort(port)
            }
            ProxySettingKey::ClientMaxBodySize => ProxySetting::ClientMaxBodySize(Some(value)), // TODO: validate
            ProxySettingKey::Disabled => {
                let disabled: bool = value
                    .parse()
                    .context("Failed to parse value as boolean: {value}")?;
                ProxySetting::Disabled(Some(disabled))
            }
            ProxySettingKey::Gzip => {
                let gzip: bool = value
                    .parse()
                    .context("Failed to parse value as boolean: {value}")?;
                ProxySetting::Gzip(Some(gzip))
            }
        })
    }
}
