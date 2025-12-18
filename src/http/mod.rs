use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::services::ServeDir;

use crate::http::healthz::{aggregate_healthz, details_healthz, self_healthz};
use crate::http::metrics::{metrics_handler, Metrics};
use crate::http::ui::ui_handler;
use crate::state::AppState;

pub mod healthz;
pub mod metrics;
pub mod ui;

pub fn router(state: Arc<AppState>, metrics: Arc<Metrics>) -> Router {
    Router::new()
        // self health
        .route("/healthz", get(self_healthz))
        .route("/healthz/self", get(self_healthz))

        // aggregated health
        .route(
            "/healthz/aggregate",
            get({
                let state = state.clone();
                move || aggregate_healthz(state.clone())
            }),
        )
        .route(
            "/healthz/aggregated",
            get({
                let state = state.clone();
                move || aggregate_healthz(state.clone())
            }),
        )

        // common k8s-friendly alias names
        .route(
            "/multi-healthz",
            get({
                let state = state.clone();
                move || aggregate_healthz(state.clone())
            }),
        )
        .route(
            "/multi-health",
            get({
                let state = state.clone();
                move || aggregate_healthz(state.clone())
            }),
        )

        // details (JSON)
        .route(
            "/healthz/details",
            get({
                let state = state.clone();
                move || details_healthz(state.clone())
            }),
        )

        // UI (HTML)
        .route(
            "/ui",
            get({
                let state = state.clone();
                move || ui_handler(state.clone())
            }),
        )

        // metrics (Prometheus)
        .route(
            "/metrics",
            get({
                let state = state.clone();
                let metrics = metrics.clone();
                move || metrics_handler(state.clone(), metrics.clone())
            }),
        )

        // static assets for UI
        .nest_service("/static", ServeDir::new("static"))
}
