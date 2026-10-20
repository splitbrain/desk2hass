//! Persistent configuration in `$XDG_CONFIG_HOME/desk2hass/config.json`.
//!
//! The file holds the data from the mobile app registration. The access token
//! is only used for the registration and is not stored.

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Connection data for one Home Assistant instance.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    /// Base URL of Home Assistant, without a trailing slash.
    pub url: String,
    /// Webhook ID that Home Assistant returned at registration.
    pub webhook_id: String,
    /// Accept any TLS certificate, for example a self-signed one.
    #[serde(default)]
    pub insecure: bool,
}

impl Config {
    /// Read the configuration file. Returns `None` when the file does not exist.
    pub fn load() -> Result<Option<Self>> {
        let path = path()?;
        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
        };
        let config =
            serde_json::from_str(&data).with_context(|| format!("parse {}", path.display()))?;
        Ok(Some(config))
    }

    /// Write the configuration file. Only the user can read it.
    pub fn save(&self) -> Result<PathBuf> {
        let path = path()?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .with_context(|| format!("write {}", path.display()))?;
        file.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
        Ok(path)
    }
}

/// Location of the configuration file.
fn path() -> Result<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => {
            let home = std::env::var_os("HOME").context("HOME is not set")?;
            PathBuf::from(home).join(".config")
        }
    };
    Ok(base.join("desk2hass").join("config.json"))
}
