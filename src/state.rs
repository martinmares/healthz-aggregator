use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::RwLock;

use crate::config::{CheckConfig, Config, DebouncePolicyConfig, ResponseProfileConfig};
use crate::notifier::{NotificationEventType, StatusChangeEvent, now_rfc3339};
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
    history_size: usize,

    global_labels: HashMap<String, String>,
    checks: Vec<CheckConfig>,
    debounce_policies: HashMap<String, DebouncePolicyConfig>,
    check_groups: HashMap<String, Vec<String>>,
    groups: HashMap<String, GroupState>,
    response_profiles: HashMap<String, ResponseProfileConfig>,
    results: RwLock<HashMap<String, CheckResult>>,
    history: RwLock<HashMap<String, VecDeque<CheckHistoryEntry>>>,
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

#[derive(Debug, Clone, Serialize)]
pub struct CheckHistoryEntry {
    pub raw_status: CheckStatus,
    pub status: CheckStatus,
    pub critical: bool,
    pub timestamp: SystemTime,
    pub duration: Option<Duration>,
    pub error: Option<String>,
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
        let mut debounce_policies = HashMap::new();
        let mut check_groups = HashMap::new();
        for c in &cfg.checks {
            let labels = Self::merge_labels(&global_labels, &c.static_labels);
            debounce_policies.insert(c.name.clone(), c.debounce.clone());
            check_groups.insert(c.name.clone(), c.groups.clone());

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
            history_size: cfg.global.history_size,
            global_labels,
            checks: cfg.checks.clone(),
            debounce_policies,
            check_groups,
            groups,
            response_profiles: cfg.response_profiles.clone(),
            results: RwLock::new(map),
            history: RwLock::new(HashMap::new()),
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

    pub async fn update(&self, r: CheckResult) -> Option<StatusChangeEvent> {
        let policy = self
            .debounce_policies
            .get(&r.name)
            .cloned()
            .unwrap_or_default();

        let mut results = self.results.write().await;
        let previous = results.get(&r.name).cloned();
        let mut history = self.history.write().await;
        let entries = history.entry(r.name.clone()).or_default();
        let effective = apply_debounce(previous.as_ref(), entries, &policy, &r);

        let history_entry = effective.last_run.map(|timestamp| CheckHistoryEntry {
            raw_status: r.status,
            status: effective.status,
            critical: effective.critical,
            timestamp,
            duration: effective.duration,
            error: effective.error.clone(),
        });

        results.insert(effective.name.clone(), effective.clone());

        if let Some(entry) = history_entry {
            entries.push_back(entry);
            while entries.len() > self.history_size {
                entries.pop_front();
            }
        }

        let previous = previous?;
        if previous.last_run.is_none() || previous.status == effective.status {
            return None;
        }

        Some(StatusChangeEvent {
            event: NotificationEventType::StatusChange,
            check_name: effective.name.clone(),
            critical: effective.critical,
            old_status: previous.status,
            new_status: effective.status,
            timestamp: effective
                .last_run
                .map(now_rfc3339)
                .unwrap_or_else(|| "-".to_string()),
            error: effective.error.clone(),
            groups: self
                .check_groups
                .get(&effective.name)
                .cloned()
                .unwrap_or_default(),
        })
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

    fn aggregate_results<I>(
        results: I,
    ) -> (bool, AggregateSummary, Vec<CheckResult>, Vec<CheckResult>)
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

    pub async fn history_for_check(&self, check_name: &str) -> Option<Vec<CheckHistoryEntry>> {
        if !self.results.read().await.contains_key(check_name) {
            return None;
        }

        let history = self.history.read().await;
        let entries = history.get(check_name).cloned().unwrap_or_default();
        Some(entries.into_iter().collect())
    }

    pub async fn recent_history_snapshot(
        &self,
        limit: usize,
    ) -> HashMap<String, Vec<CheckHistoryEntry>> {
        let history = self.history.read().await;
        history
            .iter()
            .map(|(check_name, entries)| {
                let start = entries.len().saturating_sub(limit);
                (
                    check_name.clone(),
                    entries.iter().skip(start).cloned().collect(),
                )
            })
            .collect()
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

fn apply_debounce(
    previous: Option<&CheckResult>,
    history: &VecDeque<CheckHistoryEntry>,
    policy: &DebouncePolicyConfig,
    current: &CheckResult,
) -> CheckResult {
    let previous_status = previous.map(|result| result.status);
    let mut effective = current.clone();

    match current.status {
        CheckStatus::Up => {
            if previous_status != Some(CheckStatus::Up)
                && consecutive_raw_matches(history, CheckStatus::Up) + 1 < policy.recover_after
            {
                if let Some(previous) = previous {
                    effective.status = previous.status;
                    effective.error = previous.error.clone();
                }
            } else {
                effective.error = None;
            }
        }
        failure_status => {
            if previous_status == Some(CheckStatus::Up)
                && consecutive_non_up(history) + 1 < policy.fail_after
            {
                effective.status = CheckStatus::Up;
                effective.error = None;
            } else {
                effective.status = failure_status;
            }
        }
    }

    effective
}

fn consecutive_raw_matches(history: &VecDeque<CheckHistoryEntry>, wanted: CheckStatus) -> usize {
    history
        .iter()
        .rev()
        .take_while(|entry| entry.raw_status == wanted)
        .count()
}

fn consecutive_non_up(history: &VecDeque<CheckHistoryEntry>) -> usize {
    history
        .iter()
        .rev()
        .take_while(|entry| entry.raw_status != CheckStatus::Up)
        .count()
}

#[cfg(test)]
mod tests {
    use super::{AppState, CheckResult, CheckStatus, sanitize_label_name};
    use crate::config::{
        CheckConfig, CheckSpec, Config, DebouncePolicyConfig, GlobalConfig, GroupConfig,
        ResponseProfileConfig, ServerConfig,
    };
    use std::{
        collections::HashMap,
        time::{Duration, SystemTime},
    };

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

        let _ = state
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
        let _ = state
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

    #[tokio::test]
    async fn keeps_only_last_n_history_entries() {
        let mut cfg = test_config();
        cfg.global.history_size = 2;
        let state = AppState::new(&cfg);

        for status in [CheckStatus::Up, CheckStatus::Warn, CheckStatus::Down] {
            let _ = state
                .update(CheckResult {
                    name: "public-api".to_string(),
                    status,
                    critical: true,
                    last_run: Some(SystemTime::now()),
                    duration: None,
                    error: None,
                    labels: HashMap::new(),
                })
                .await;
        }

        let history = state
            .history_for_check("public-api")
            .await
            .expect("history should exist");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].status, CheckStatus::Warn);
        assert_eq!(history[1].status, CheckStatus::Down);
    }

    #[tokio::test]
    async fn applies_fail_and_recover_debounce_thresholds() {
        let mut cfg = test_config();
        cfg.checks[0].debounce = DebouncePolicyConfig {
            fail_after: 2,
            recover_after: 2,
        };
        let state = AppState::new(&cfg);

        let _ = state
            .update(CheckResult {
                name: "public-api".to_string(),
                status: CheckStatus::Up,
                critical: true,
                last_run: Some(SystemTime::now()),
                duration: None,
                error: None,
                labels: HashMap::new(),
            })
            .await;
        let _ = state
            .update(CheckResult {
                name: "public-api".to_string(),
                status: CheckStatus::Up,
                critical: true,
                last_run: Some(SystemTime::now()),
                duration: None,
                error: None,
                labels: HashMap::new(),
            })
            .await;

        let _ = state
            .update(CheckResult {
                name: "public-api".to_string(),
                status: CheckStatus::Down,
                critical: true,
                last_run: Some(SystemTime::now()),
                duration: None,
                error: Some("first failure".to_string()),
                labels: HashMap::new(),
            })
            .await;
        assert_eq!(
            state.get("public-api").await.expect("check exists").status,
            CheckStatus::Up
        );

        let _ = state
            .update(CheckResult {
                name: "public-api".to_string(),
                status: CheckStatus::Down,
                critical: true,
                last_run: Some(SystemTime::now()),
                duration: None,
                error: Some("second failure".to_string()),
                labels: HashMap::new(),
            })
            .await;
        assert_eq!(
            state.get("public-api").await.expect("check exists").status,
            CheckStatus::Down
        );

        let _ = state
            .update(CheckResult {
                name: "public-api".to_string(),
                status: CheckStatus::Up,
                critical: true,
                last_run: Some(SystemTime::now()),
                duration: None,
                error: None,
                labels: HashMap::new(),
            })
            .await;
        assert_eq!(
            state.get("public-api").await.expect("check exists").status,
            CheckStatus::Down
        );

        let _ = state
            .update(CheckResult {
                name: "public-api".to_string(),
                status: CheckStatus::Up,
                critical: true,
                last_run: Some(SystemTime::now()),
                duration: None,
                error: None,
                labels: HashMap::new(),
            })
            .await;
        assert_eq!(
            state.get("public-api").await.expect("check exists").status,
            CheckStatus::Up
        );
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
                history_size: 20,
            },
            metrics: None,
            notifications: None,
            response_profiles,
            groups,
            checks: vec![
                CheckConfig {
                    name: "public-api".to_string(),
                    critical: true,
                    static_labels: HashMap::new(),
                    groups: vec!["public-lb".to_string()],
                    debounce: DebouncePolicyConfig::default(),
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
                    debounce: DebouncePolicyConfig::default(),
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
