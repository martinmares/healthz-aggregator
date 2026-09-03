use crate::config::{CheckConfig, CheckSpec};
use anyhow::Result;

pub mod file;
pub mod http;
pub mod http_json;
mod json_path;
pub mod oracle;
pub mod postgres;
pub mod tcp;
pub mod tls_cert;
pub mod tls_client;

pub async fn run_check(cfg: &CheckConfig) -> Result<()> {
    match &cfg.spec {
        CheckSpec::Tcp { .. } => tcp::run(cfg).await,
        CheckSpec::Http { .. } => http::run(cfg).await,
        CheckSpec::HttpJson { .. } => http_json::run(cfg).await,
        CheckSpec::TlsCert { .. } => tls_cert::run(cfg).await,
        CheckSpec::Postgres { .. } => postgres::run(cfg).await,
        CheckSpec::File { .. } => file::run(cfg).await,
        CheckSpec::Oracle { .. } => oracle::run(cfg).await,
    }
}
