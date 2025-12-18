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
    /// How often to refresh the cached check results.
    #[serde(with = "humantime_serde")]
    pub refresh_interval: Duration,

    /// Optional concurrency limit for running checks.
    /// If not set, all checks run concurrently.
    #[serde(default)]
    pub max_concurrency: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetricsConfig {
    pub namespace: Option<String>,
    pub name: Option<String>,

    /// Global labels applied to all metrics (env/cluster/...).
    /// These are also merged into each check's labels (check-level labels override).
    pub static_labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CheckConfig {
    pub name: String,

    /// If false: a failing check becomes WARN (does not fail the aggregate health).
    #[serde(default = "default_true")]
    pub critical: bool,

    /// Per-check labels. Merged with metrics.static_labels.
    #[serde(default)]
    pub static_labels: HashMap<String, String>,

    #[serde(flatten)]
    pub spec: CheckSpec,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CheckSpec {
    Tcp {
        host: String,
        port: u16,

        #[serde(with = "humantime_serde", default)]
        timeout: Option<Duration>,
    },

    Http {
        url: String,
        method: Option<String>,
        headers: Option<HashMap<String, String>>,

        #[serde(with = "humantime_serde", default)]
        timeout: Option<Duration>,

        tls_verify: Option<bool>,
        status_code: Option<u16>,
        expected_body_substring: Option<String>,
        expected_body_regex: Option<String>,
    },

    HttpJson {
        url: String,
        method: Option<String>,
        headers: Option<HashMap<String, String>>,

        #[serde(with = "humantime_serde", default)]
        timeout: Option<Duration>,

        tls_verify: Option<bool>,
        status_code: Option<u16>,

        /// Small JSONPath subset: $.a.b[0].c
        json_path: Option<String>,

        /// Compare extracted value as string.
        expected_value: Option<String>,

        /// Match extracted value as string.
        expected_regex: Option<String>,
    },

    /// TLS certificate expiry check (TCP + TLS handshake + read leaf cert).
    /// Note: Implementation depends on TLS stack; keep config stable.
    TlsCert {
        /// Either host/port or url may be provided.
        host: Option<String>,
        port: Option<u16>,
        url: Option<String>,

        /// SNI override (defaults to host).
        sni: Option<String>,

        #[serde(with = "humantime_serde", default)]
        timeout: Option<Duration>,

        tls_verify: Option<bool>,

        /// Mark DOWN/WARN if remaining days < min_days_remaining.
        min_days_remaining: Option<f64>,
    },

    /// Postgres SQL check (planned; keep schema compatible with legacy Ruby config).
    Postgres {
        host: String,
        port: Option<u16>,
        database: String,
        username: String,
        password: Option<String>,

        #[serde(with = "humantime_serde", default)]
        connect_timeout: Option<Duration>,

        tls: Option<bool>,
        ignore_invalid_cert: Option<bool>,

        query: String,

        /// Compare first column of first row as string.
        expected_scalar: Option<String>,
        expected_contains: Option<String>,
        expected_regex: Option<String>,
    },

    File {
        /// File path on the local filesystem.
        path: String,
        /// Optional format hint: "text" (default) or "json".
        #[serde(default)]
        format: Option<String>,
        /// Optional JSON path (only a small subset is supported, see docs).
        #[serde(default)]
        json_path: Option<String>,
        /// Exact match.
        #[serde(default)]
        expected_value: Option<String>,
        /// Substring match.
        #[serde(default, alias = "expected_substring")]
        expected_contains: Option<String>,
        /// Regex match.
        #[serde(default)]
        expected_regex: Option<String>,
    },

    /// Oracle SQL check (planned; likely feature-gated).
    Oracle {
        /// Either provide a full Oracle connect string...
        #[serde(default)]
        connection_string: Option<String>,
        /// ...or build it from host/port/service_name or sid.
        #[serde(default)]
        host: Option<String>,
        #[serde(default)]
        port: Option<u16>,
        #[serde(default)]
        service_name: Option<String>,
        #[serde(default)]
        sid: Option<String>,

        username: String,
        password: Option<String>,
        #[serde(default, with = "humantime_serde")]
        connect_timeout: Option<Duration>,
        query: String,
        expected_scalar: Option<String>,
        expected_contains: Option<String>,
        expected_regex: Option<String>,
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
