use serde::Deserialize;
use std::{collections::HashMap, fs, time::Duration};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub global: GlobalConfig,
    pub metrics: Option<MetricsConfig>,
    pub checks: Vec<CheckConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    /// Bind address for HTTP server, e.g. "0.0.0.0:8998"
    pub bind: String,
}
#[derive(Debug, Deserialize, Clone)]
pub struct GlobalConfig {
    #[serde(with = "humantime_serde")]
    pub refresh_interval: Duration,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetricsConfig {
    pub namespace: Option<String>,
    pub name: Option<String>,
    pub static_labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CheckConfig {
    pub name: String,

    #[serde(default = "default_true")]
    pub critical: bool,

    #[serde(default)]
    pub static_labels: HashMap<String, String>,

    #[serde(flatten)]
    pub spec: CheckSpec,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CheckSpec {
    Http {
        url: String,
        method: Option<String>,

        #[serde(with = "humantime_serde", default)]
        timeout: Option<Duration>,

        tls_verify: Option<bool>,
        status_code: Option<u16>,
        expected_body_substring: Option<String>,
    },
}

fn default_true() -> bool {
    true
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("HEALTHZ_CONFIG").unwrap_or_else(|_| "config.yaml".to_string());
        let raw = fs::read_to_string(&path)?;
        Ok(serde_yaml_ng::from_str(&raw)?)
    }
}
