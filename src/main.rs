mod checks;
mod config;
mod http;
mod scheduler;
mod state;

use crate::{config::Config, http::metrics::Metrics, state::AppState};
use clap::Parser;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Parser)]
#[command(name = "healthcheck-aggregator")]
struct Cli {
    /// Open browser at http://<bind>/ui
    #[arg(long)]
    open: bool,

    /// Open browser at the provided URL
    #[arg(long, value_name = "URL")]
    open_url: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
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
        cfg.global.default_timeout,
    );

    let metrics_cfg = cfg
        .metrics
        .clone()
        .unwrap_or(crate::config::MetricsConfig {
            namespace: None,
            name: None,
            static_labels: None,
        });

    let metrics = Arc::new(Metrics::new(&metrics_cfg, &cfg.checks));

    let app = http::router(state, metrics);
    let bind_addr = &cfg.server.bind;
    let listener = TcpListener::bind(bind_addr).await?;

    tracing::info!(bind_addr = %bind_addr, "healthcheck-aggregator HTTP server started");

    if cli.open || cli.open_url.is_some() {
        let url = cli.open_url.or_else(|| ui_url_from_bind(bind_addr));
        if let Some(url) = url {
            let _ = webbrowser::open(&url);
        }
    }

    axum::serve(listener, app).await?;

    Ok(())
}

fn ui_url_from_bind(bind_addr: &str) -> Option<String> {
    let mut parts = bind_addr.rsplitn(2, ':');
    let port = parts.next()?;
    let host = parts.next().unwrap_or("localhost");
    let host = match host {
        "0.0.0.0" | "127.0.0.1" | "::" | "[::]" => "localhost",
        other => other,
    };
    Some(format!("http://{host}:{port}/ui"))
}

#[cfg(test)]
mod tests {
    use super::ui_url_from_bind;

    #[test]
    fn ui_url_from_bind_localhost() {
        assert_eq!(
            ui_url_from_bind("0.0.0.0:8080").as_deref(),
            Some("http://localhost:8080/ui")
        );
        assert_eq!(
            ui_url_from_bind("[::]:9000").as_deref(),
            Some("http://localhost:9000/ui")
        );
        assert_eq!(
            ui_url_from_bind("127.0.0.1:1234").as_deref(),
            Some("http://localhost:1234/ui")
        );
    }
}
