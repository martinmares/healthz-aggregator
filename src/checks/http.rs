use crate::config::{CheckConfig, CheckSpec};
use anyhow::{Result, anyhow};
use reqwest::{Client, Method};

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let spec = match &cfg.spec {
        CheckSpec::Http { .. } => &cfg.spec,
    };

    let (url, method, timeout, tls_verify, status_code, expected_body_substring) = match spec {
        CheckSpec::Http {
            url,
            method,
            timeout,
            tls_verify,
            status_code,
            expected_body_substring,
        } => (
            url,
            method.as_deref().unwrap_or("GET"),
            *timeout,
            *tls_verify,
            *status_code,
            expected_body_substring,
        ),
    };

    let mut builder = Client::builder();
    if tls_verify == Some(false) {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder.build()?;

    let method: Method = method.parse()?;
    let mut req = client.request(method, url);

    if let Some(t) = timeout {
        req = req.timeout(t);
    }

    let resp = req.send().await?;

    if let Some(code) = status_code {
        if resp.status().as_u16() != code {
            return Err(anyhow!("unexpected status {}", resp.status()));
        }
    }

    if let Some(substr) = expected_body_substring {
        let body = resp.text().await?;
        if !body.contains(substr) {
            return Err(anyhow!("response body mismatch"));
        }
    }

    Ok(())
}
