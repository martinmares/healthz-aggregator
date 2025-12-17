use axum::{Router, routing::get};
use std::sync::Arc;

use crate::http::metrics::{Metrics, metrics_handler};
use crate::state::AppState;

pub mod metrics;

pub fn router(state: Arc<AppState>, metrics: Arc<Metrics>) -> Router {
    Router::new().route(
        "/metrics",
        get({
            let state = state.clone();
            let metrics = metrics.clone();
            move || metrics_handler(state.clone(), metrics.clone())
        }),
    )
}
