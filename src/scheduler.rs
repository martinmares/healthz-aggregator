use crate::{
    checks,
    state::{AppState, CheckResult, CheckStatus},
};
use futures::future::join_all;
use std::{sync::Arc, time::Instant};
use tokio::sync::Semaphore;

pub fn spawn(state: Arc<AppState>, interval: std::time::Duration, max_concurrency: Option<usize>) {
    tracing::info!(interval = ?interval, max_concurrency = ?max_concurrency, "scheduler started");

    let semaphore = max_concurrency.map(|n| Arc::new(Semaphore::new(n)));

    tokio::spawn(async move {
        loop {
            run_once(state.clone(), semaphore.clone()).await;
            tokio::time::sleep(interval).await;
        }
    });
}

async fn run_once(state: Arc<AppState>, semaphore: Option<Arc<Semaphore>>) {
    tracing::debug!("scheduler tick");

    let futures = state.check_configs().into_iter().map(|cfg| {
        let state = state.clone();
        let semaphore = semaphore.clone();

        async move {
            // Optional concurrency limit
            let _permit = match semaphore {
                Some(ref sem) => Some(sem.acquire().await.expect("semaphore closed")),
                None => None,
            };

            let start = Instant::now();
            tracing::info!(check = %cfg.name, "check started");

            let res = checks::run_check(&cfg).await;
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
            state.update(result);
        }
    });

    join_all(futures).await;
}
