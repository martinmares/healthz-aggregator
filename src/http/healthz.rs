use axum::{Json, extract::Path, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use std::sync::Arc;
use time::OffsetDateTime;

use crate::state::{AppState, CheckResult};

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
    let (ok, summary, failed, warn) = state.aggregate_snapshot();

    let body = AggregateResponse {
        status: if ok { "ok" } else { "failed" },
        summary,
        failed,
        warn,
        timestamp: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap(),
    };

    let status = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status, Json(body))
}

pub async fn details_healthz(state: Arc<AppState>) -> impl IntoResponse {
    let uptime = state.uptime();

    let body = DetailsResponse {
        uptime,
        timestamp: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap(),
        checks: state.snapshot(),
    };

    (StatusCode::OK, Json(body))
}

pub async fn details_healthz_one(
    state: Arc<AppState>,
    Path(check_name): Path<String>,
) -> impl IntoResponse {
    let results = state.snapshot(); // nebo líp: state.get(&check_name)
    if let Some(r) = results.into_iter().find(|x| x.name == check_name) {
        return (StatusCode::OK, Json(r)).into_response();
    }
    (StatusCode::NOT_FOUND, "check not found").into_response()
}
