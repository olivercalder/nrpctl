use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::process::Command;

#[derive(clap::ValueEnum, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SSLSelection {
    False,
    NoRedirect,
    Redirect,
}

impl fmt::Display for SSLSelection {
    // Need to implement std::Display so that the SSL configuration option can be displayed via
    // commands.
    // TODO: add thorough tests.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            SSLSelection::False => "false",
            SSLSelection::NoRedirect => "no-redirect",
            SSLSelection::Redirect => "redirect",
        };
        write!(f, "{}", name)
    }
}

impl SSLSelection {
    pub fn is_true(&self) -> bool {
        match self {
            SSLSelection::False => false,
            SSLSelection::NoRedirect => true,
            SSLSelection::Redirect => true,
        }
    }
}

impl std::str::FromStr for SSLSelection {
    // Need to implement FromStr so the SSL configuration can be parsed by the `set` command.
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<SSLSelection> {
        match input {
            "false" => Ok(SSLSelection::False),
            "no-redirect" => Ok(SSLSelection::NoRedirect),
            "redirect" => Ok(SSLSelection::Redirect),
            _ => Err(anyhow!(
                "Matching variant not found; expected one of: [false, no-redirect, redirect]"
            )),
        }
    }
}

pub fn handle_ssl_selection(
    listen_domain: &str,
    ssl_selection: SSLSelection,
) -> Result<Box<dyn FnOnce() -> Result<String>>> {
    if let SSLSelection::False = ssl_selection {
        return Ok(Box::new(|| Ok(String::new()))); // nothing to do or roll back
    }
    // Check that letsencrypt configuration exists as expected.
    // TODO: allow configuration for these to live elsewhere
    let include = "/etc/letsencrypt/options-ssl-nginx.conf";
    let ssl_dhparam = "/etc/letsencrypt/ssl-dhparams.pem";
    for ssl_config_path in [include, ssl_dhparam] {
        ensure!(
            fs::exists(ssl_config_path).context(
                "Failed to check the existence of SSL configuration file: {ssl_config_path}"
            )?,
            "SSL configuration file not found: {ssl_config_path}"
        )
    }

    if !Command::new("certbot")
        .arg("certonly")
        .arg("--nginx")
        .arg("--non-interactive")
        .arg("--no-eff-email")
        .arg("--domains")
        .arg(listen_domain)
        .status()
        .context("Failed to execute certbot")?
        .success()
    {
        return Err(anyhow!(
            "Error when obtaining ssl certificate for {listen_domain}"
        ));
    }

    // By default, certbot saves the cert and key at:
    // - /etc/letsencrypt/live/{listen_domain}/fullchain.pem
    // - /etc/letsencrypt/live/{listen_domain}/privkey.pem
    // TODO: allow configuration of alternative locations

    let ssl_certificate = format!("/etc/letsencrypt/live/{}/fullchain.pem", listen_domain);
    let ssl_certificate_key = format!("/etc/letsencrypt/live/{}/privkey.pem", listen_domain);
    for cert_filepath in [&ssl_certificate, &ssl_certificate_key] {
        ensure!(
            fs::exists(cert_filepath).context(
                "Failed to check the existence of SSL certificate file: {cert_filepath}"
            )?,
            "SSL certificate file not found: {cert_filepath}"
        )
    }

    // Once SSL cert is generated and validated, it's not worth revoking/deleting it, since it can
    // be reused if SSL is successfully enabled in the future.
    Ok(Box::new(|| Ok(String::new())))
}
