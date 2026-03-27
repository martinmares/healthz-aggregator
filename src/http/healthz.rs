use axum::{
    Json,
    extract::Path,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::sync::Arc;
use time::OffsetDateTime;

use crate::{
    config::{ResponseProfileConfig, ResponseSpecConfig},
    state::{AppState, CheckResult},
};

#[derive(Serialize)]
struct AggregateResponse {
    status: &'static str,
    summary: crate::state::AggregateSummary,
    failed: Vec<CheckResult>,
    warn: Vec<CheckResult>,
    timestamp: String,
}

#[derive(Serialize)]
struct DetailsResponse {
    uptime: String,
    timestamp: String,
    checks: Vec<CheckResult>,
}

pub async fn self_healthz() -> impl IntoResponse {
    StatusCode::OK
}

pub async fn aggregate_healthz(state: Arc<AppState>) -> impl IntoResponse {
    let (ok, summary, failed, warn) = state.aggregate_snapshot().await;
    aggregate_response(&state, ok, summary, failed, warn, None, None)
}

pub async fn group_aggregate_healthz(
    state: Arc<AppState>,
    Path(group_name): Path<String>,
) -> impl IntoResponse {
    let Some((ok, summary, failed, warn)) = state.aggregate_snapshot_for_group(&group_name).await
    else {
        return (StatusCode::NOT_FOUND, "group not found").into_response();
    };

    aggregate_response(
        &state,
        ok,
        summary,
        failed,
        warn,
        Some(group_name),
        None,
    )
}

pub async fn group_profile_healthz(
    state: Arc<AppState>,
    Path((group_name, profile_name)): Path<(String, String)>,
) -> impl IntoResponse {
    if !state.group_allows_profile(&group_name, &profile_name) {
        return (StatusCode::NOT_FOUND, "profile not found for group").into_response();
    }

    let Some((ok, summary, failed, warn)) = state.aggregate_snapshot_for_group(&group_name).await
    else {
        return (StatusCode::NOT_FOUND, "group not found").into_response();
    };

    aggregate_response(
        &state,
        ok,
        summary,
        failed,
        warn,
        Some(group_name),
        Some(profile_name),
    )
}

pub async fn details_healthz(state: Arc<AppState>) -> impl IntoResponse {
    let uptime = state.uptime();

    let body = DetailsResponse {
        uptime,
        timestamp: now_rfc3339(),
        checks: state.snapshot().await,
    };

    (StatusCode::OK, Json(body)).into_response()
}

pub async fn group_details_healthz(
    state: Arc<AppState>,
    Path(group_name): Path<String>,
) -> impl IntoResponse {
    let Some(mut checks) = state.snapshot_for_group(&group_name).await else {
        return (StatusCode::NOT_FOUND, "group not found").into_response();
    };

    checks.sort_by(|a, b| a.name.cmp(&b.name));

    let body = DetailsResponse {
        uptime: state.uptime(),
        timestamp: now_rfc3339(),
        checks,
    };

    (StatusCode::OK, Json(body)).into_response()
}

pub async fn details_healthz_one(
    state: Arc<AppState>,
    Path(check_name): Path<String>,
) -> impl IntoResponse {
    if let Some(r) = state.get(&check_name).await {
        return (StatusCode::OK, Json(r)).into_response();
    }
    (StatusCode::NOT_FOUND, "check not found").into_response()
}

fn aggregate_response(
    state: &AppState,
    ok: bool,
    summary: crate::state::AggregateSummary,
    failed: Vec<CheckResult>,
    warn: Vec<CheckResult>,
    group_name: Option<String>,
    requested_profile: Option<String>,
) -> Response {
    let profile_name = requested_profile.or_else(|| {
        group_name
            .as_deref()
            .and_then(|name| state.default_profile_name_for_group(name))
            .map(str::to_string)
    });

    let profile = profile_name
        .as_deref()
        .and_then(|profile_name| state.response_profile(profile_name));

    if let Some(profile) = profile {
        return custom_aggregate_response(profile, ok);
    }

    let body = AggregateResponse {
        status: if ok { "ok" } else { "failed" },
        summary,
        failed,
        warn,
        timestamp: now_rfc3339(),
    };

    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(body)).into_response()
}

fn custom_aggregate_response(profile: &ResponseProfileConfig, ok: bool) -> Response {
    let spec = if ok { &profile.ok } else { &profile.fail };
    let status = spec
        .status_code
        .and_then(|code| StatusCode::from_u16(code).ok())
        .unwrap_or(if ok {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        });

    let body = spec
        .body
        .clone()
        .unwrap_or_else(|| if ok { "OK".to_string() } else { "FAILED".to_string() });
    let content_type = spec
        .content_type
        .clone()
        .unwrap_or_else(|| default_content_type(spec));

    let mut response = (status, body).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("text/plain; charset=utf-8")),
    );
    response
}

fn default_content_type(spec: &ResponseSpecConfig) -> String {
    match spec.body.as_deref() {
        Some(body) if body.trim_start().starts_with('{') => "application/json".to_string(),
        _ => "text/plain; charset=utf-8".to_string(),
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "-".into())
}
