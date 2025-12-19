use askama::Template;
use axum::{
    Json,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Serialize;
use std::{sync::Arc, time::SystemTime};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::state::{AppState, CheckResult, CheckStatus};

#[derive(Debug, Clone)]
struct CheckRow {
    name: String,
    status: String,
    critical: bool,
    last_run: String,

    // Plain string is used for conditional rendering.
    error: String,
    // Pre-rendered popover HTML (escaped, safe).
    error_html: String,
    labels_html: String,
}

#[derive(Template)]
#[template(path = "ui.html")]
struct UiTemplate {
    title: String,
    aggregate_ok: bool,
    now: String,
    uptime: String,
    refresh_interval: String,
    refresh_interval_secs: u64,

    summary_total: usize,
    summary_up: usize,
    summary_warn: usize,
    summary_down: usize,
    summary_critical_down: usize,

    checks: Vec<CheckRow>,
}

#[derive(Debug, Clone, Serialize)]
struct UiCheckSnapshot {
    name: String,
    status: String,
    critical: bool,
    last_run: Option<String>,
    error: String,
    labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct UiSnapshotResponse {
    aggregate_ok: bool,
    now: String,
    uptime: String,
    refresh_interval: String,

    summary_total: usize,
    summary_up: usize,
    summary_warn: usize,
    summary_down: usize,
    summary_critical_down: usize,

    checks: Vec<UiCheckSnapshot>,
}

fn fmt_rfc3339_opt(st: Option<SystemTime>) -> Option<String> {
    let st = st?;
    let d = st.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    let dt = OffsetDateTime::from_unix_timestamp(d.as_secs() as i64).ok()?;
    dt.format(&Rfc3339).ok()
}

fn fmt_rfc3339_or_dash(st: Option<SystemTime>) -> String {
    fmt_rfc3339_opt(st).unwrap_or_else(|| "-".into())
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn status_str(s: CheckStatus) -> String {
    match s {
        CheckStatus::Up => "up".into(),
        CheckStatus::Warn => "warn".into(),
        CheckStatus::Down => "down".into(),
    }
}

fn labels_to_vec(r: &CheckResult) -> Vec<String> {
    if r.labels.is_empty() {
        return vec![];
    }

    let mut pairs: Vec<String> = r
        .labels
        .iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    pairs.sort();
    pairs
}

fn labels_to_popover_html(r: &CheckResult) -> String {
    let pairs = labels_to_vec(r);
    if pairs.is_empty() {
        return String::new();
    }

    // Build small HTML list (each line escaped) – stable rendering in popover.
    // NOTE: This HTML is embedded into an *attribute value* (data-bs-content).
    // Using single quotes inside prevents broken markup when rendered server-side.
    let mut out = String::from("<div class='hc-popover-lines'>");
    for p in pairs {
        out.push_str("<div>- ");
        out.push_str(&html_escape(&p));
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}

fn error_to_popover_html(error: &str) -> String {
    if error.trim().is_empty() {
        return String::new();
    }
    let safe = html_escape(error)
        .replace("\r\n", "\n")
        .replace('\n', "<br>");
    // Same attribute-embedding note as above: prefer single quotes inside.
    format!("<div class='hc-popover-lines'>{safe}</div>")
}

pub async fn ui_handler(state: Arc<AppState>) -> impl IntoResponse {
    let (aggregate_ok, summary, _failed, _warn) = state.aggregate_snapshot();

    let mut rows: Vec<CheckRow> = state
        .snapshot()
        .into_iter()
        .map(|r| {
            let error = r.error.clone().unwrap_or_default();
            CheckRow {
                name: r.name.clone(),
                status: status_str(r.status),
                critical: r.critical,
                last_run: fmt_rfc3339_or_dash(r.last_run),
                error_html: error_to_popover_html(&error),
                labels_html: labels_to_popover_html(&r),
                error,
            }
        })
        .collect();

    rows.sort_by(|a, b| a.name.cmp(&b.name));

    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "-".into());

    let refresh_interval_secs = state.refresh_interval().as_secs().max(1);

    let tpl = UiTemplate {
        title: "healthcheck-aggregator".into(),
        aggregate_ok,
        now,
        uptime: state.uptime(),
        refresh_interval: humantime::format_duration(state.refresh_interval()).to_string(),
        refresh_interval_secs,

        summary_total: summary.total,
        summary_up: summary.up,
        summary_warn: summary.warn,
        summary_down: summary.down,
        summary_critical_down: summary.critical_down,

        checks: rows,
    };

    match tpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => Html(format!("template error: {e}")).into_response(),
    }
}

pub async fn ui_snapshot_handler(state: Arc<AppState>) -> impl IntoResponse {
    let (aggregate_ok, summary, _failed, _warn) = state.aggregate_snapshot();

    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "-".into());

    let mut checks: Vec<UiCheckSnapshot> = state
        .snapshot()
        .into_iter()
        .map(|r| UiCheckSnapshot {
            name: r.name.clone(),
            status: status_str(r.status),
            critical: r.critical,
            last_run: fmt_rfc3339_opt(r.last_run),
            error: r.error.clone().unwrap_or_default(),
            labels: labels_to_vec(&r),
        })
        .collect();
    checks.sort_by(|a, b| a.name.cmp(&b.name));

    let body = UiSnapshotResponse {
        aggregate_ok,
        now,
        uptime: state.uptime(),
        refresh_interval: humantime::format_duration(state.refresh_interval()).to_string(),

        summary_total: summary.total,
        summary_up: summary.up,
        summary_warn: summary.warn,
        summary_down: summary.down,
        summary_critical_down: summary.critical_down,

        checks,
    };

    (StatusCode::OK, Json(body))
}
