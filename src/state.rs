use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::RwLock;

use crate::config::{CheckConfig, Config};
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
    results: RwLock<HashMap<String, CheckResult>>,
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

        Self {
            start: Instant::now(),
            refresh_interval,
            global_labels,
            checks: cfg.checks.clone(),
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

        let mut up = 0;
        let mut warn = 0;
        let mut down = 0;
        let mut critical_down = 0;

        let mut failed = Vec::new();
        let mut warned = Vec::new();

        for r in results.values() {
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
    use super::sanitize_label_name;

    #[test]
    fn sanitize_label_name_basic() {
        assert_eq!(sanitize_label_name("Env"), "env");
        assert_eq!(sanitize_label_name("a-b.c"), "a_b_c");
        assert_eq!(sanitize_label_name("9lives"), "_9lives");
        assert_eq!(sanitize_label_name(""), "_");
    }
}
