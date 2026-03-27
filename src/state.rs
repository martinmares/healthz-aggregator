use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::RwLock;

use crate::config::{CheckConfig, Config, ResponseProfileConfig};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Up,
    Down,
    Warn,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub critical: bool,
    pub last_run: Option<SystemTime>,
    pub duration: Option<Duration>,
    pub error: Option<String>,

    /// Merged labels: metrics.static_labels + check.static_labels (check overrides).
    /// Keys are sanitized to Prometheus label name rules.
    pub labels: HashMap<String, String>,
}

pub struct AppState {
    start: Instant,
    refresh_interval: Duration,

    global_labels: HashMap<String, String>,
    checks: Vec<CheckConfig>,
    groups: HashMap<String, GroupState>,
    response_profiles: HashMap<String, ResponseProfileConfig>,
    results: RwLock<HashMap<String, CheckResult>>,
}

#[derive(Debug, Clone)]
pub struct GroupState {
    pub default_profile: Option<String>,
    profile_names: HashSet<String>,
    check_names: HashSet<String>,
}

#[derive(Serialize)]
pub struct AggregateSummary {
    pub total: usize,
    pub up: usize,
    pub warn: usize,
    pub down: usize,
    pub critical_down: usize,
}

fn sanitize_label_name(name: &str) -> String {
    let mut out: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if out.is_empty() {
        return "_".to_string();
    }

    let first = out.chars().next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        out.insert(0, '_');
    }
    out
}

impl AppState {
    pub fn new(cfg: &Config) -> Self {
        let refresh_interval = cfg.global.refresh_interval;

        let mut global_labels = cfg
            .metrics
            .as_ref()
            .and_then(|m| m.static_labels.clone())
            .unwrap_or_default();

        // sanitize global label keys
        let global_labels: HashMap<String, String> = global_labels
            .drain()
            .map(|(k, v)| (sanitize_label_name(&k), v))
            .collect();

        let mut map = HashMap::new();
        for c in &cfg.checks {
            let labels = Self::merge_labels(&global_labels, &c.static_labels);

            map.insert(
                c.name.clone(),
                CheckResult {
                    name: c.name.clone(),
                    status: CheckStatus::Warn,
                    critical: c.critical,
                    last_run: None,
                    duration: None,
                    error: Some("not yet executed".into()),
                    labels,
                },
            );
        }

        let groups = cfg
            .groups
            .iter()
            .map(|(name, group_cfg)| {
                let check_names = cfg
                    .checks
                    .iter()
                    .filter(|check| check.groups.iter().any(|group| group == name))
                    .map(|check| check.name.clone())
                    .collect();
                (
                    name.clone(),
                    GroupState {
                        default_profile: group_cfg.default_profile.clone(),
                        profile_names: group_cfg.profiles.iter().cloned().collect(),
                        check_names,
                    },
                )
            })
            .collect();

        Self {
            start: Instant::now(),
            refresh_interval,
            global_labels,
            checks: cfg.checks.clone(),
            groups,
            response_profiles: cfg.response_profiles.clone(),
            results: RwLock::new(map),
        }
    }

    pub fn refresh_interval(&self) -> Duration {
        self.refresh_interval
    }


    pub fn merge_labels(
        global: &HashMap<String, String>,
        per_check: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut out = global.clone();
        for (k, v) in per_check {
            out.insert(sanitize_label_name(k), v.clone());
        }
        out
    }

    pub fn labels_for_check(&self, cfg: &CheckConfig) -> HashMap<String, String> {
        Self::merge_labels(&self.global_labels, &cfg.static_labels)
    }

    pub fn check_configs(&self) -> Vec<CheckConfig> {
        self.checks.clone()
    }

    pub fn response_profile(&self, name: &str) -> Option<&ResponseProfileConfig> {
        self.response_profiles.get(name)
    }

    pub fn default_profile_name_for_group(&self, group_name: &str) -> Option<&str> {
        self.groups
            .get(group_name)
            .and_then(|group| group.default_profile.as_deref())
    }

    pub fn profile_names_for_group(&self, group_name: &str) -> Option<Vec<String>> {
        let group = self.groups.get(group_name)?;
        let mut names: Vec<String> = group.profile_names.iter().cloned().collect();
        names.sort();
        Some(names)
    }

    pub fn group_allows_profile(&self, group_name: &str, profile_name: &str) -> bool {
        self.groups
            .get(group_name)
            .map(|group| group.profile_names.contains(profile_name))
            .unwrap_or(false)
    }

    pub fn group_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.groups.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn group_check_count(&self, name: &str) -> Option<usize> {
        self.groups.get(name).map(|group| group.check_names.len())
    }

    pub async fn update(&self, r: CheckResult) {
        self.results.write().await.insert(r.name.clone(), r);
    }

    pub async fn snapshot(&self) -> Vec<CheckResult> {
        self.results.read().await.values().cloned().collect()
    }

    pub async fn aggregate_snapshot(
        &self,
    ) -> (bool, AggregateSummary, Vec<CheckResult>, Vec<CheckResult>) {
        let results = self.results.read().await;
        Self::aggregate_results(results.values().cloned())
    }

    pub async fn aggregate_snapshot_for_group(
        &self,
        group_name: &str,
    ) -> Option<(bool, AggregateSummary, Vec<CheckResult>, Vec<CheckResult>)> {
        let group = self.groups.get(group_name)?;
        let results = self.results.read().await;
        Some(Self::aggregate_results(
            results
                .values()
                .filter(|result| group.check_names.contains(&result.name))
                .cloned(),
        ))
    }

    pub async fn snapshot_for_group(&self, group_name: &str) -> Option<Vec<CheckResult>> {
        let group = self.groups.get(group_name)?;
        let results = self.results.read().await;
        Some(
            results
                .values()
                .filter(|result| group.check_names.contains(&result.name))
                .cloned()
                .collect(),
        )
    }

    fn aggregate_results<I>(results: I) -> (bool, AggregateSummary, Vec<CheckResult>, Vec<CheckResult>)
    where
        I: IntoIterator<Item = CheckResult>,
    {
        let results: Vec<CheckResult> = results.into_iter().collect();

        let mut up = 0;
        let mut warn = 0;
        let mut down = 0;
        let mut critical_down = 0;

        let mut failed = Vec::new();
        let mut warned = Vec::new();

        for r in &results {
            match r.status {
                CheckStatus::Up => up += 1,
                CheckStatus::Warn => {
                    warn += 1;
                    warned.push(r.clone());
                }
                CheckStatus::Down => {
                    down += 1;
                    if r.critical {
                        critical_down += 1;
                        failed.push(r.clone());
                    } else {
                        warned.push(r.clone());
                    }
                }
            }
        }

        let summary = AggregateSummary {
            total: results.len(),
            up,
            warn,
            down,
            critical_down,
        };

        let ok = critical_down == 0;
        (ok, summary, failed, warned)
    }

    pub async fn get(&self, check_name: &str) -> Option<CheckResult> {
        self.results.read().await.get(check_name).cloned()
    }

    pub fn uptime(&self) -> String {
        // Human-friendly uptime for UI. Keep it stable and readable for L2.
        // Examples: "7.428 s", "3m 12s", "2h 05m", "1d 4h".
        let d = self.start.elapsed();
        let secs = d.as_secs_f64();

        if secs < 60.0 {
            return format!("{:.3} s", secs);
        }

        if secs < 60.0 * 60.0 {
            let total = secs.floor() as u64;
            let m = total / 60;
            let s = total % 60;
            return format!("{m}m {s}s");
        }

        if secs < 60.0 * 60.0 * 24.0 {
            let total = secs.floor() as u64;
            let h = total / 3600;
            let m = (total % 3600) / 60;
            return format!("{h}h {m:02}m");
        }

        let total = secs.floor() as u64;
        let days = total / 86_400;
        let h = (total % 86_400) / 3600;
        format!("{days}d {h}h")
    }
}

#[cfg(test)]
mod tests {
    use super::{AppState, CheckResult, CheckStatus, sanitize_label_name};
    use crate::config::{
        CheckConfig, CheckSpec, Config, GlobalConfig, GroupConfig, ResponseProfileConfig,
        ServerConfig,
    };
    use std::{collections::HashMap, time::Duration};

    #[test]
    fn sanitize_label_name_basic() {
        assert_eq!(sanitize_label_name("Env"), "env");
        assert_eq!(sanitize_label_name("a-b.c"), "a_b_c");
        assert_eq!(sanitize_label_name("9lives"), "_9lives");
        assert_eq!(sanitize_label_name(""), "_");
    }

    #[tokio::test]
    async fn aggregates_only_checks_in_selected_group() {
        let state = AppState::new(&test_config());

        state
            .update(CheckResult {
                name: "public-api".to_string(),
                status: CheckStatus::Up,
                critical: true,
                last_run: None,
                duration: None,
                error: None,
                labels: HashMap::new(),
            })
            .await;
        state
            .update(CheckResult {
                name: "internal-db".to_string(),
                status: CheckStatus::Down,
                critical: true,
                last_run: None,
                duration: None,
                error: Some("db down".to_string()),
                labels: HashMap::new(),
            })
            .await;

        let (ok, summary, failed, warned) = state
            .aggregate_snapshot_for_group("public-lb")
            .await
            .expect("group should exist");

        assert!(ok);
        assert_eq!(summary.total, 1);
        assert_eq!(summary.up, 1);
        assert_eq!(summary.down, 0);
        assert!(failed.is_empty());
        assert!(warned.is_empty());
    }

    fn test_config() -> Config {
        let mut response_profiles = HashMap::new();
        response_profiles.insert("hw-lb".to_string(), ResponseProfileConfig::default());

        let mut groups = HashMap::new();
        groups.insert(
            "public-lb".to_string(),
            GroupConfig {
                default_profile: Some("hw-lb".to_string()),
                profiles: vec!["hw-lb".to_string()],
            },
        );
        groups.insert("internal-ui".to_string(), GroupConfig::default());

        Config {
            server: ServerConfig {
                bind: "127.0.0.1:8998".to_string(),
            },
            global: GlobalConfig {
                refresh_interval: Duration::from_secs(30),
                default_timeout: None,
                max_concurrency: None,
            },
            metrics: None,
            response_profiles,
            groups,
            checks: vec![
                CheckConfig {
                    name: "public-api".to_string(),
                    critical: true,
                    static_labels: HashMap::new(),
                    groups: vec!["public-lb".to_string()],
                    spec: CheckSpec::Tcp {
                        host: "localhost".to_string(),
                        port: 80,
                        timeout: None,
                    },
                },
                CheckConfig {
                    name: "internal-db".to_string(),
                    critical: true,
                    static_labels: HashMap::new(),
                    groups: vec!["internal-ui".to_string()],
                    spec: CheckSpec::Tcp {
                        host: "localhost".to_string(),
                        port: 5432,
                        timeout: None,
                    },
                },
            ],
        }
    }
}
