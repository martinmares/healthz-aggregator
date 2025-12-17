use axum::response::IntoResponse;
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::MetricsConfig;
use crate::state::{AppState, CheckStatus};

fn normalize_namespace(ns: Option<&str>) -> String {
    match ns {
        None | Some("") => "".to_string(),
        Some(s) if s.ends_with('_') => s.to_string(),
        Some(s) => format!("{}_", s),
    }
}

pub struct Metrics {
    registry: Registry,
    health_up: GaugeVec,
    duration: GaugeVec,
    last_run: GaugeVec,
    label_keys: Vec<String>,
    static_labels: HashMap<String, String>,
}

impl Metrics {
    pub fn new(cfg: &MetricsConfig) -> Self {
        let ns = normalize_namespace(cfg.namespace.as_deref());
        let name = cfg.name.as_deref().unwrap_or("health");

        let mut label_keys = vec!["check".to_string()];
        let static_labels = cfg.static_labels.clone().unwrap_or_default();

        for k in static_labels.keys() {
            label_keys.push(k.clone());
        }

        let label_refs: Vec<&str> = label_keys.iter().map(|s| s.as_str()).collect();

        let registry = Registry::new();

        let health_up = GaugeVec::new(
            Opts::new(format!("{}{}_up", ns, name), "Health check status"),
            &label_refs,
        )
        .unwrap();

        let duration = GaugeVec::new(
            Opts::new(
                format!("{}{}_duration_seconds", ns, name),
                "Health check duration",
            ),
            &label_refs,
        )
        .unwrap();

        let last_run = GaugeVec::new(
            Opts::new(
                format!("{}{}_last_run_timestamp", ns, name),
                "Last execution timestamp",
            ),
            &label_refs,
        )
        .unwrap();

        registry.register(Box::new(health_up.clone())).unwrap();
        registry.register(Box::new(duration.clone())).unwrap();
        registry.register(Box::new(last_run.clone())).unwrap();

        Self {
            registry,
            health_up,
            duration,
            last_run,
            label_keys,
            static_labels,
        }
    }

    pub fn update_from_state(&self, state: &AppState) {
        let snapshot = state.snapshot();

        for r in snapshot {
            let mut labels: HashMap<&str, &str> = HashMap::new();

            for k in &self.label_keys {
                if k == "check" {
                    labels.insert("check", r.name.as_str());
                } else if let Some(v) = self.static_labels.get(k) {
                    labels.insert(k.as_str(), v.as_str());
                }
            }

            let up = match r.status {
                CheckStatus::Up | CheckStatus::Warn => 1.0,
                CheckStatus::Down => 0.0,
            };

            self.health_up.with(&labels).set(up);

            if let Some(d) = r.duration {
                self.duration.with(&labels).set(d.as_secs_f64());
            }

            if let Some(ts) = r.last_run {
                if let Ok(epoch) = ts.duration_since(std::time::UNIX_EPOCH) {
                    self.last_run.with(&labels).set(epoch.as_secs() as f64);
                }
            }
        }
    }

    pub fn encode(&self) -> String {
        let families = self.registry.gather();
        let encoder = TextEncoder::new();
        let mut buf = Vec::new();
        encoder.encode(&families, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }
}

/// HTTP handler - POZOR NA JMÉNO (ne `metrics`)
pub async fn metrics_handler(state: Arc<AppState>, metrics: Arc<Metrics>) -> impl IntoResponse {
    metrics.update_from_state(&state);
    metrics.encode()
}
