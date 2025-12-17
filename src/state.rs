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
}
