use anyhow::Result;
use hive_protocol::{DEFAULT_HOST_PORT, DEFAULT_HUB_PORT, DEFAULT_WEB_PORT};
use serde::{Deserialize, Serialize};

use crate::bind;
use crate::home::HiveHome;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_host_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_hub_port")]
    pub port: u16,
    #[serde(default = "default_poll")]
    pub poll_secs: u64,
    #[serde(default = "default_host_port")]
    pub host_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default = "default_web_root")]
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientToml {
    #[serde(default)]
    pub hub: String,
    #[serde(default)]
    pub terminal: String,
}

fn default_bind() -> String {
    bind::tailscale_v4().unwrap_or_else(|| "127.0.0.1".into())
}

fn default_host_port() -> u16 {
    DEFAULT_HOST_PORT
}

fn default_hub_port() -> u16 {
    DEFAULT_HUB_PORT
}

fn default_web_port() -> u16 {
    DEFAULT_WEB_PORT
}

fn default_web_root() -> String {
    String::new()
}

fn default_poll() -> u64 {
    5
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            port: default_host_port(),
        }
    }
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            port: default_hub_port(),
            poll_secs: default_poll(),
            host_port: default_host_port(),
        }
    }
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            port: default_web_port(),
            root: default_web_root(),
        }
    }
}

impl HostConfig {
    pub fn load(home: &HiveHome) -> Result<Self> {
        let path = home.host_toml();
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }
}

impl HubConfig {
    pub fn load(home: &HiveHome) -> Result<Self> {
        let path = home.hub_toml();
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    }
}

impl WebConfig {
    pub fn load(home: &HiveHome) -> Result<Self> {
        let mut cfg = if home.web_toml().exists() {
            toml::from_str(&std::fs::read_to_string(home.web_toml())?)?
        } else {
            Self::default()
        };
        if cfg.root.is_empty() {
            cfg.root = home.web_dist_dir().display().to_string();
        }
        Ok(cfg)
    }

    pub fn save(&self, home: &HiveHome) -> Result<()> {
        let text = toml::to_string_pretty(self)?;
        std::fs::write(home.web_toml(), text)?;
        Ok(())
    }
}

impl ClientToml {
    pub fn load(home: &HiveHome) -> Self {
        let path = home.client_toml();
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        toml::from_str(&text).unwrap_or_default()
    }
}
