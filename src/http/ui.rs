use askama::Template;
use axum::{
    extract::Query,
    Json,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
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
    active_group: String,
    active_scope_label: String,
    scope_help: String,
    scope_health_href: String,
    details_href: String,
    snapshot_href: String,
    scope_default_profile: String,
    scope_default_profile_href: String,
    has_profile_testing: bool,
    group_options: Vec<GroupOption>,
    profile_options: Vec<ProfileOption>,

    summary_total: usize,
    summary_up: usize,
    summary_warn: usize,
    summary_down: usize,
    summary_critical_down: usize,

    checks: Vec<CheckRow>,
}

#[derive(Debug, Clone)]
struct GroupOption {
    value: String,
    label: String,
    selected: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ProfileOption {
    value: String,
    label: String,
    selected: bool,
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
    active_group: String,
    active_scope_label: String,
    scope_help: String,
    scope_health_href: String,
    details_href: String,
    scope_default_profile: String,
    scope_default_profile_href: String,
    has_profile_testing: bool,
    profile_options: Vec<ProfileOption>,

    summary_total: usize,
    summary_up: usize,
    summary_warn: usize,
    summary_down: usize,
    summary_critical_down: usize,

    checks: Vec<UiCheckSnapshot>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UiQuery {
    group: Option<String>,
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

pub async fn ui_handler(state: Arc<AppState>, Query(query): Query<UiQuery>) -> impl IntoResponse {
    let Some(model) = build_ui_model(&state, query.group).await else {
        return (StatusCode::NOT_FOUND, "group not found").into_response();
    };
    let refresh_interval_secs = state.refresh_interval().as_secs().max(1);
    let tpl = UiTemplate {
        title: "healthz-aggregator".into(),
        aggregate_ok: model.aggregate_ok,
        now: model.now,
        uptime: model.uptime,
        refresh_interval: humantime::format_duration(state.refresh_interval()).to_string(),
        refresh_interval_secs,
        active_group: model.active_group,
        active_scope_label: model.active_scope_label,
        scope_help: model.scope_help,
        scope_health_href: model.scope_health_href,
        details_href: model.details_href,
        snapshot_href: model.snapshot_href,
        scope_default_profile: model.scope_default_profile,
        scope_default_profile_href: model.scope_default_profile_href,
        has_profile_testing: model.has_profile_testing,
        group_options: model.group_options,
        profile_options: model.profile_options,

        summary_total: model.summary_total,
        summary_up: model.summary_up,
        summary_warn: model.summary_warn,
        summary_down: model.summary_down,
        summary_critical_down: model.summary_critical_down,

        checks: model.checks,
    };

    match tpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => Html(format!("template error: {e}")).into_response(),
    }
}

pub async fn ui_snapshot_handler(
    state: Arc<AppState>,
    Query(query): Query<UiQuery>,
) -> impl IntoResponse {
    let Some(model) = build_ui_model(&state, query.group).await else {
        return (StatusCode::NOT_FOUND, "group not found").into_response();
    };
    let body = UiSnapshotResponse {
        aggregate_ok: model.aggregate_ok,
        now: model.now,
        uptime: model.uptime,
        refresh_interval: humantime::format_duration(state.refresh_interval()).to_string(),
        active_group: model.active_group,
        active_scope_label: model.active_scope_label,
        scope_help: model.scope_help,
        scope_health_href: model.scope_health_href,
        details_href: model.details_href,
        scope_default_profile: model.scope_default_profile,
        scope_default_profile_href: model.scope_default_profile_href,
        has_profile_testing: model.has_profile_testing,
        profile_options: model.profile_options,

        summary_total: model.summary_total,
        summary_up: model.summary_up,
        summary_warn: model.summary_warn,
        summary_down: model.summary_down,
        summary_critical_down: model.summary_critical_down,

        checks: model.snapshot_checks,
    };

    (StatusCode::OK, Json(body)).into_response()
}

struct UiModel {
    aggregate_ok: bool,
    now: String,
    uptime: String,
    active_group: String,
    active_scope_label: String,
    scope_help: String,
    scope_health_href: String,
    details_href: String,
    snapshot_href: String,
    scope_default_profile: String,
    scope_default_profile_href: String,
    has_profile_testing: bool,
    group_options: Vec<GroupOption>,
    profile_options: Vec<ProfileOption>,
    summary_total: usize,
    summary_up: usize,
    summary_warn: usize,
    summary_down: usize,
    summary_critical_down: usize,
    snapshot_checks: Vec<UiCheckSnapshot>,
    checks: Vec<CheckRow>,
}

async fn build_ui_model(state: &AppState, requested_group: Option<String>) -> Option<UiModel> {
    let selected_group = requested_group
        .map(|group| group.trim().to_string())
        .filter(|group| !group.is_empty());

    let (
        aggregate_ok,
        summary,
        mut results,
        scope_health_href,
        details_href,
        active_group,
        active_scope_label,
        scope_default_profile,
        scope_default_profile_href,
        has_profile_testing,
        profile_options,
    ) =
        if let Some(group_name) = selected_group.clone() {
            let (aggregate_ok, summary, _failed, _warn) =
                state.aggregate_snapshot_for_group(&group_name).await?;
            let results = state.snapshot_for_group(&group_name).await?;
            let default_profile = state
                .default_profile_name_for_group(&group_name)
                .map(str::to_string)
                .unwrap_or_else(|| "built-in-json".to_string());
            (
                aggregate_ok,
                summary,
                results,
                format!("/groups/{group_name}/healthz"),
                format!("/groups/{group_name}/healthz/details"),
                group_name.clone(),
                format!("Group: {group_name}"),
                default_profile.clone(),
                format!("/groups/{group_name}/healthz"),
                true,
                build_profile_options(state, &group_name, &default_profile),
            )
        } else {
            let (aggregate_ok, summary, _failed, _warn) = state.aggregate_snapshot().await;
            let results = state.snapshot().await;
            (
                aggregate_ok,
                summary,
                results,
                "/healthz/aggregate".to_string(),
                "/healthz/details".to_string(),
                String::new(),
                "All checks".to_string(),
                String::new(),
                String::new(),
                false,
                Vec::new(),
            )
        };

    let mut snapshot_checks: Vec<UiCheckSnapshot> = results
        .iter()
        .map(|r| UiCheckSnapshot {
            name: r.name.clone(),
            status: status_str(r.status),
            critical: r.critical,
            last_run: fmt_rfc3339_opt(r.last_run),
            error: r.error.clone().unwrap_or_default(),
            labels: labels_to_vec(r),
        })
        .collect();
    snapshot_checks.sort_by(|a, b| a.name.cmp(&b.name));

    let mut rows: Vec<CheckRow> = results
        .drain(..)
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

    let snapshot_href = if active_group.is_empty() {
        "/ui/api/snapshot".to_string()
    } else {
        format!("/ui/api/snapshot?group={active_group}")
    };

    Some(UiModel {
        aggregate_ok,
        now: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "-".into()),
        uptime: state.uptime(),
        active_group: active_group.clone(),
        active_scope_label,
        scope_help: "Groups are logical health views. One check can belong to more than one group."
            .to_string(),
        scope_health_href,
        details_href,
        snapshot_href,
        scope_default_profile,
        scope_default_profile_href,
        has_profile_testing,
        group_options: build_group_options(state, &active_group),
        profile_options,
        summary_total: summary.total,
        summary_up: summary.up,
        summary_warn: summary.warn,
        summary_down: summary.down,
        summary_critical_down: summary.critical_down,
        snapshot_checks,
        checks: rows,
    })
}

fn build_group_options(state: &AppState, active_group: &str) -> Vec<GroupOption> {
    let mut options = vec![GroupOption {
        value: String::new(),
        label: format!("All checks ({})", state.check_configs().len()),
        selected: active_group.is_empty(),
    }];
    options.extend(state.group_names().into_iter().map(|name| GroupOption {
        selected: name == active_group,
        value: name.clone(),
        label: format!(
            "{} ({})",
            name,
            state.group_check_count(&name).unwrap_or_default()
        ),
    }));
    options
}

fn build_profile_options(
    state: &AppState,
    group_name: &str,
    default_profile: &str,
) -> Vec<ProfileOption> {
    let mut options = vec![ProfileOption {
        value: "__default__".to_string(),
        label: format!("default endpoint ({default_profile})"),
        selected: true,
    }];

    if let Some(profile_names) = state.profile_names_for_group(group_name) {
        options.extend(profile_names.into_iter().map(|name| ProfileOption {
            selected: false,
            value: name.clone(),
            label: name,
        }));
    }

    options
}
