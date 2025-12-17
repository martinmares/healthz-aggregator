use crate::{
    checks,
    state::{AppState, CheckResult, CheckStatus},
};
use futures::future::join_all;
use std::{sync::Arc, time::Instant};

pub fn spawn(state: Arc<AppState>, interval: std::time::Duration) {
    tracing::info!(
        interval = ?interval,
        "scheduler started"
    );

    tokio::spawn(async move {
        loop {
            run_once(state.clone()).await;

            tracing::info!(
                interval = ?interval,
                "scheduler sleeping"
            );

            tokio::time::sleep(interval).await;
        }
    });
}

async fn run_once(state: Arc<AppState>) {
    tracing::info!("scheduler tick");
    let futures = state.check_configs().into_iter().map(|cfg| {
        let state = state.clone();
        async move {
            let start = Instant::now();

            tracing::info!(
                check = %cfg.name,
                "check started"
            );

            let res = checks::run_check(&cfg).await;
            let duration = start.elapsed();

            match &res {
                Ok(_) => tracing::info!(
                    check = %cfg.name,
                    duration = ?duration,
                    "check finished OK"
                ),
                Err(err) => tracing::warn!(
                    check = %cfg.name,
                    duration = ?duration,
                    error = %err,
                    "check failed"
                ),
            };

            let result = match res {
                Ok(_) => CheckResult {
                    name: cfg.name.clone(),
                    status: CheckStatus::Up,
                    critical: cfg.critical,
                    last_run: Some(std::time::SystemTime::now()),
                    duration: Some(duration),
                    error: None,
                    labels: cfg.static_labels.clone(),
                },
                Err(e) => CheckResult {
                    name: cfg.name.clone(),
                    status: if cfg.critical {
                        CheckStatus::Down
                    } else {
                        CheckStatus::Warn
                    },
                    critical: cfg.critical,
                    last_run: Some(std::time::SystemTime::now()),
                    duration: Some(duration),
                    error: Some(e.to_string()),
                    labels: cfg.static_labels.clone(),
                },
            };

            state.update(result);
        }
    });

    join_all(futures).await;
}
