use anyhow::{Context, bail};
use serde::Deserialize;
use std::{collections::HashMap, fs, time::Duration};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub global: GlobalConfig,
    pub metrics: Option<MetricsConfig>,
    #[serde(default)]
    pub response_profiles: HashMap<String, ResponseProfileConfig>,
    #[serde(default)]
    pub groups: HashMap<String, GroupConfig>,
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

    /// Fallback timeout for any check without its own timeout.
    #[serde(with = "humantime_serde", default)]
    pub default_timeout: Option<Duration>,

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

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ResponseProfileConfig {
    #[serde(default)]
    pub ok: ResponseSpecConfig,
    #[serde(default)]
    pub fail: ResponseSpecConfig,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ResponseSpecConfig {
    pub status_code: Option<u16>,
    pub content_type: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GroupConfig {
    #[serde(default, alias = "response_profile")]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: Vec<String>,
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

    #[serde(default)]
    pub groups: Vec<String>,

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
    pub fn load_from_path(path: Option<&str>) -> anyhow::Result<Self> {
        let path = path.unwrap_or("config.yaml");
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading config file from {path}"))?;
        let cfg: Self = serde_yaml_ng::from_str(&raw).context("parsing YAML config")?;
        cfg.validate().context("validating config")?;
        Ok(cfg)
    }

    fn validate(&self) -> anyhow::Result<()> {
        for (group_name, group_cfg) in &self.groups {
            if let Some(profile_name) = &group_cfg.default_profile
                && !self.response_profiles.contains_key(profile_name)
            {
                bail!(
                    "group '{group_name}' references unknown default_profile '{profile_name}'"
                );
            }

            for profile_name in &group_cfg.profiles {
                if !self.response_profiles.contains_key(profile_name) {
                    bail!(
                        "group '{group_name}' references unknown profile '{profile_name}'"
                    );
                }
            }

            if let Some(default_profile) = &group_cfg.default_profile
                && !group_cfg.profiles.iter().any(|profile| profile == default_profile)
            {
                bail!(
                    "group '{group_name}' default_profile '{default_profile}' must also be present in profiles"
                );
            }
        }

        for check in &self.checks {
            for group_name in &check.groups {
                if !self.groups.contains_key(group_name) {
                    bail!(
                        "check '{}' references unknown group '{}'",
                        check.name,
                        group_name
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn rejects_unknown_group_reference() {
        let yaml = r#"
server:
  bind: 127.0.0.1:8998
global:
  refresh_interval: 30s
checks:
  - name: api
    groups: [missing]
    type: tcp
    host: localhost
    port: 80
"#;

        let cfg: Config = serde_yaml_ng::from_str(yaml).expect("config should parse");
        let err = cfg.validate().expect_err("config should fail validation");
        assert!(err.to_string().contains("unknown group 'missing'"));
    }

    #[test]
    fn rejects_unknown_response_profile_reference() {
        let yaml = r#"
server:
  bind: 127.0.0.1:8998
global:
  refresh_interval: 30s
groups:
  public:
    default_profile: missing
checks: []
"#;

        let cfg: Config = serde_yaml_ng::from_str(yaml).expect("config should parse");
        let err = cfg.validate().expect_err("config should fail validation");
        assert!(err.to_string().contains("unknown default_profile 'missing'"));
    }

    #[test]
    fn rejects_default_profile_not_in_whitelist() {
        let yaml = r#"
server:
  bind: 127.0.0.1:8998
global:
  refresh_interval: 30s
response_profiles:
  json:
    ok:
      body: '{"status":"ok"}'
groups:
  public:
    default_profile: json
    profiles: []
checks: []
"#;

        let cfg: Config = serde_yaml_ng::from_str(yaml).expect("config should parse");
        let err = cfg.validate().expect_err("config should fail validation");
        assert!(err.to_string().contains("must also be present in profiles"));
    }
}
