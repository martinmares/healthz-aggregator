use axum::response::IntoResponse;
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use std::{collections::HashSet, sync::Arc};

use crate::config::{CheckConfig, MetricsConfig};
use crate::state::{AppState, CheckStatus};

fn sanitize_metric_name(name: &str) -> String {
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
        out = "_".into();
    }

    let first = out.chars().next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        out.insert(0, '_');
    }

    out
}

fn sanitize_label_name(name: &str) -> String {
    sanitize_metric_name(name)
}

fn namespace_prefix(ns: Option<&str>) -> String {
    match ns {
        None => "".into(),
        Some(s) => {
            let s = s.trim();
            if s.is_empty() {
                return "".into();
            }
            let s = sanitize_metric_name(s);
            if s.ends_with('_') {
                s
            } else {
                format!("{}_", s)
            }
        }
    }
}

pub struct Metrics {
    registry: Registry,
    health_up: GaugeVec,
    duration: GaugeVec,
    last_run: GaugeVec,

    /// Label names in a fixed order (first is always "check").
    label_names: Vec<String>,

    /// Same order as label_names, but without the leading "check".
    extra_label_keys: Vec<String>,
}

impl Metrics {
    pub fn new(cfg: &MetricsConfig, checks: &[CheckConfig]) -> Self {
        let ns = namespace_prefix(cfg.namespace.as_deref());
        let name = sanitize_metric_name(cfg.name.as_deref().unwrap_or("health"));

        // Build union of label keys: global metrics.static_labels + per-check static_labels.
        // (Keys are sanitized to Prometheus label-name rules.)
        let mut union: HashSet<String> = HashSet::new();

        if let Some(sl) = &cfg.static_labels {
            for k in sl.keys() {
                union.insert(sanitize_label_name(k));
            }
        }

        for c in checks {
            for k in c.static_labels.keys() {
                union.insert(sanitize_label_name(k));
            }
        }

        let mut extra_label_keys: Vec<String> = union.into_iter().collect();
        extra_label_keys.sort();

        let mut label_names = vec!["check".to_string()];
        label_names.extend(extra_label_keys.iter().cloned());

        let label_refs: Vec<&str> = label_names.iter().map(|s| s.as_str()).collect();

        let registry = Registry::new();

        let health_up = GaugeVec::new(
            Opts::new(format!("{}{}_up", ns, name), "Health check status (1=up, 0=down)"),
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
            label_names,
            extra_label_keys,
        }
    }

    pub fn update_from_state(&self, state: &AppState) {
        let snapshot = state.snapshot();

        for r in snapshot {
            // Keep labels fixed-length; missing keys become empty string.
            let mut values: Vec<&str> = Vec::with_capacity(self.label_names.len());
            values.push(r.name.as_str());
            for k in &self.extra_label_keys {
                let v = r.labels.get(k).map(|s| s.as_str()).unwrap_or("");
                values.push(v);
            }

            let up = match r.status {
                CheckStatus::Up | CheckStatus::Warn => 1.0,
                CheckStatus::Down => 0.0,
            };

            self.health_up.with_label_values(&values).set(up);

            if let Some(d) = r.duration {
                self.duration.with_label_values(&values).set(d.as_secs_f64());
            }

            if let Some(ts) = r.last_run {
                if let Ok(epoch) = ts.duration_since(std::time::UNIX_EPOCH) {
                    self.last_run
                        .with_label_values(&values)
                        .set(epoch.as_secs() as f64);
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

/// HTTP handler
pub async fn metrics_handler(state: Arc<AppState>, metrics: Arc<Metrics>) -> impl IntoResponse {
    metrics.update_from_state(&state);
    metrics.encode()
}
