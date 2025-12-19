mod checks;
mod config;
mod http;
mod scheduler;
mod state;

use crate::{config::Config, http::metrics::Metrics, state::AppState};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();

    let cfg = Config::load()?;
    let state = Arc::new(AppState::new(&cfg));

    scheduler::spawn(
        state.clone(),
        cfg.global.refresh_interval,
        cfg.global.max_concurrency,
    );

    let metrics_cfg = cfg
        .metrics
        .clone()
        .unwrap_or_else(|| crate::config::MetricsConfig {
            namespace: None,
            name: None,
            static_labels: None,
        });

    let metrics = Arc::new(Metrics::new(&metrics_cfg, &cfg.checks));

    let app = http::router(state, metrics);
    let bind_addr = &cfg.server.bind;
    let listener = TcpListener::bind(bind_addr).await?;

    tracing::info!(bind_addr = %bind_addr, "healthcheck-aggregator HTTP server started");

    axum::serve(listener, app).await?;

    Ok(())
}
