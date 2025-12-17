use crate::state::{AppState, CheckStatus};
use axum::{Json, http::StatusCode, response::IntoResponse};
use std::sync::Arc;

pub async fn self_health() -> impl IntoResponse {
    StatusCode::OK
}

pub async fn aggregate(state: Arc<AppState>) -> impl IntoResponse {
    let agg = state.aggregate_status();
    match agg.status {
        CheckStatus::Up => StatusCode::OK,
        CheckStatus::Down => StatusCode::SERVICE_UNAVAILABLE,
        CheckStatus::Warn => StatusCode::OK,
    }
}

pub async fn details(state: Arc<AppState>) -> impl IntoResponse {
    Json(state.snapshot())
}
