use crate::{
    checks,
    config::CheckSpec,
    state::{AppState, CheckResult, CheckStatus},
};
use anyhow::anyhow;
use futures::future::join_all;
use std::{sync::Arc, time::Instant};
use tokio::sync::Semaphore;

pub fn spawn(
    state: Arc<AppState>,
    interval: std::time::Duration,
    max_concurrency: Option<usize>,
    default_timeout: Option<std::time::Duration>,
) {
    tracing::info!(interval = ?interval, max_concurrency = ?max_concurrency, "scheduler started");

    let semaphore = max_concurrency.map(|n| Arc::new(Semaphore::new(n)));

    tokio::spawn(async move {
        loop {
            run_once(state.clone(), semaphore.clone(), default_timeout).await;
            tokio::time::sleep(interval).await;
        }
    });
}

async fn run_once(
    state: Arc<AppState>,
    semaphore: Option<Arc<Semaphore>>,
    default_timeout: Option<std::time::Duration>,
) {
    tracing::debug!("scheduler tick");

    let futures = state.check_configs().into_iter().map(|cfg| {
        let state = state.clone();
        let semaphore = semaphore.clone();

        async move {
            // Optional concurrency limit
            let _permit = match semaphore {
                Some(ref sem) => match sem.acquire().await {
                    Ok(permit) => Some(permit),
                    Err(_) => {
                        tracing::warn!(check = %cfg.name, "semaphore closed, skipping check");
                        return;
                    }
                },
                None => None,
            };

            let start = Instant::now();
            tracing::info!(check = %cfg.name, "check started");

            let timeout = check_timeout(&cfg).or(default_timeout);
            let res = match timeout {
                Some(t) => match tokio::time::timeout(t, checks::run_check(&cfg)).await {
                    Ok(r) => r,
                    Err(_) => Err(anyhow!(
                        "check timed out after {}",
                        humantime::format_duration(t)
                    )),
                },
                None => checks::run_check(&cfg).await,
            };
            let duration = start.elapsed();

            let (status, error) = match res {
                Ok(_) => (CheckStatus::Up, None),
                Err(e) => {
                    // Keep the whole error chain (hugely useful for TLS/DB failures)
                    let s = format!("{:#}", e);
                    if cfg.critical {
                        (CheckStatus::Down, Some(s))
                    } else {
                        (CheckStatus::Warn, Some(s))
                    }
                }
            };

            let labels = state.labels_for_check(&cfg);

            let result = CheckResult {
                name: cfg.name.clone(),
                status,
                critical: cfg.critical,
                last_run: Some(std::time::SystemTime::now()),
                duration: Some(duration),
                error,
                labels,
            };

            tracing::info!(check = %cfg.name, status = ?result.status, duration = ?duration, "check finished");
            state.update(result).await;
        }
    });

    join_all(futures).await;
}

fn check_timeout(cfg: &crate::config::CheckConfig) -> Option<std::time::Duration> {
    match &cfg.spec {
        CheckSpec::Tcp { timeout, .. } => *timeout,
        CheckSpec::Http { timeout, .. } => *timeout,
        CheckSpec::HttpJson { timeout, .. } => *timeout,
        CheckSpec::TlsCert { timeout, .. } => *timeout,
        CheckSpec::Postgres { connect_timeout, .. } => *connect_timeout,
        CheckSpec::Oracle { connect_timeout, .. } => *connect_timeout,
        CheckSpec::File { .. } => None,
    }
}
