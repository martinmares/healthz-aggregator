use askama::Template;
use axum::response::{Html, IntoResponse};
use std::{sync::Arc, time::SystemTime};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::state::{AppState, CheckResult, CheckStatus};

#[derive(Debug, Clone)]
struct CheckRow {
    name: String,
    status: String,
    critical: bool,
    last_run: String,
    duration: String,
    error: String,
    labels: String,
}

#[derive(Template)]
#[template(path = "ui.html")]

struct UiTemplate {
    title: String,
    aggregate_ok: bool,
    now: String,
    uptime: String,
    refresh_interval: String,

    summary_total: usize,
    summary_up: usize,
    summary_warn: usize,
    summary_down: usize,
    summary_critical_down: usize,

    checks: Vec<CheckRow>,
}

fn fmt_rfc3339(st: Option<SystemTime>) -> String {
    let Some(st) = st else {
        return "-".into();
    };

    let Ok(d) = st.duration_since(SystemTime::UNIX_EPOCH) else {
        return "-".into();
    };
    let Ok(dt) = OffsetDateTime::from_unix_timestamp(d.as_secs() as i64) else {
        return "-".into();
    };
    dt.format(&Rfc3339).unwrap_or_else(|_| "-".into())
}

fn fmt_duration(d: Option<std::time::Duration>) -> String {
    let Some(d) = d else {
        return "—".into();
    };

    let us = d.as_secs_f64() * 1_000_000.0;

    if us >= 1_000_000.0 {
        format!("{:.3} s", us / 1_000_000.0)
    } else if us >= 1_000.0 {
        format!("{:.3} ms", us / 1_000.0)
    } else if us >= 1.0 {
        format!("{:.3} µs", us)
    } else {
        // ultra-rychlé věci, ať to nezmizí úplně
        format!("{} ns", d.as_nanos())
    }
}

fn status_str(s: CheckStatus) -> String {
    match s {
        CheckStatus::Up => "up".into(),
        CheckStatus::Warn => "warn".into(),
        CheckStatus::Down => "down".into(),
    }
}

fn labels_to_string(r: &CheckResult) -> String {
    let mut pairs: Vec<String> = r
        .labels
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    pairs.sort();
    pairs.join(",")
}

pub async fn ui_handler(state: Arc<AppState>) -> impl IntoResponse {
    let (aggregate_ok, summary, _failed, _warn) = state.aggregate_snapshot();

    let mut rows: Vec<CheckRow> = state
        .snapshot()
        .into_iter()
        .map(|r| CheckRow {
            name: r.name.clone(),
            status: status_str(r.status),
            critical: r.critical,
            last_run: fmt_rfc3339(r.last_run),
            duration: fmt_duration(r.duration),
            error: r.error.clone().unwrap_or_else(|| "".into()),
            labels: labels_to_string(&r),
        })
        .collect();

    rows.sort_by(|a, b| a.name.cmp(&b.name));

    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "-".into());

    let tpl = UiTemplate {
        title: "multi-healthz".into(),
        aggregate_ok,
        now,
        uptime: state.uptime(),
        refresh_interval: humantime::format_duration(state.refresh_interval()).to_string(),

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
