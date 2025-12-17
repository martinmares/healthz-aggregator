use crate::config::{CheckConfig, CheckSpec};
use anyhow::Result;

pub mod http;

pub async fn run_check(cfg: &CheckConfig) -> Result<()> {
    match &cfg.spec {
        CheckSpec::Http { .. } => http::run(cfg).await,
    }
}
