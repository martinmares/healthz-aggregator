use axum::{Router, routing::get};
use std::sync::Arc;

use crate::http::healthz::{aggregate_healthz, self_healthz};
use crate::http::metrics::{Metrics, metrics_handler};
use crate::state::AppState;

pub mod healthz;
pub mod metrics;

pub fn router(state: Arc<AppState>, metrics: Arc<Metrics>) -> Router {
    Router::new()
        // healthz
        .route("/healthz/self", get(self_healthz))
        .route(
            "/healthz/aggregate",
            get({
                let state = state.clone();
                move || aggregate_healthz(state.clone())
            }),
        )
        // metrics
        .route(
            "/metrics",
            get({
                let state = state.clone();
                let metrics = metrics.clone();
                move || metrics_handler(state.clone(), metrics.clone())
            }),
        )
        .route(
            "/healthz/details",
            get({
                let state = state.clone();
                move || healthz::details_healthz(state.clone())
            }),
        )
}
