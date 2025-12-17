use std::{
    collections::HashMap,
    sync::RwLock,
    time::{Duration, Instant, SystemTime},
};

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
    pub labels: HashMap<String, String>,
}

pub struct AppState {
    #[allow(dead_code)]
    start: Instant,
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

impl AppState {
    pub fn new(cfg: &Config) -> Self {
        let mut map = HashMap::new();
        for c in &cfg.checks {
            map.insert(
                c.name.clone(),
                CheckResult {
                    name: c.name.clone(),
                    status: CheckStatus::Warn,
                    critical: c.critical,
                    last_run: None,
                    duration: None,
                    error: Some("not yet executed".into()),
                    labels: c.static_labels.clone(),
                },
            );
        }

        Self {
            start: Instant::now(),
            checks: cfg.checks.clone(),
            results: RwLock::new(map),
        }
    }

    pub fn check_configs(&self) -> Vec<CheckConfig> {
        self.checks.clone()
    }

    pub fn update(&self, r: CheckResult) {
        self.results.write().unwrap().insert(r.name.clone(), r);
    }

    pub fn snapshot(&self) -> Vec<CheckResult> {
        self.results.read().unwrap().values().cloned().collect()
    }

    /// True = aggregate OK
    pub fn aggregate_ok(&self) -> bool {
        let results = self.results.read().unwrap();

        for r in results.values() {
            if r.critical && r.status == CheckStatus::Down {
                return false;
            }
        }

        true
    }

    pub fn aggregate_snapshot(
        &self,
    ) -> (bool, AggregateSummary, Vec<CheckResult>, Vec<CheckResult>) {
        let results = self.results.read().unwrap();

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

    pub fn uptime(&self) -> String {
        humantime::format_duration(self.start.elapsed()).to_string()
    }
}
