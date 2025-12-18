use crate::config::{CheckConfig, CheckSpec};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue}, Client, Method};

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let (
        url,
        method,
        headers,
        timeout,
        tls_verify,
        status_code,
        expected_body_substring,
        expected_body_regex,
    ) = match &cfg.spec {
        CheckSpec::Http {
            url,
            method,
            headers,
            timeout,
            tls_verify,
            status_code,
            expected_body_substring,
            expected_body_regex,
        } => (
            url.as_str(),
            method.as_deref().unwrap_or("GET"),
            headers,
            *timeout,
            *tls_verify,
            *status_code,
            expected_body_substring.as_deref(),
            expected_body_regex.as_deref(),
        ),
        _ => return Err(anyhow!("invalid check spec for http")),
    };

    let mut builder = Client::builder();
    if tls_verify == Some(false) {
        builder = builder.danger_accept_invalid_certs(true);
    }
    let client = builder.build().context("building reqwest client")?;

    let method: Method = method.parse().context("parsing HTTP method")?;
    let mut req = client.request(method, url);

    if let Some(t) = timeout {
        req = req.timeout(t);
    }

    if let Some(hdrs) = headers {
        let mut map = HeaderMap::new();
        for (k, v) in hdrs {
            let name = HeaderName::from_bytes(k.as_bytes())
                .with_context(|| format!("invalid header name: {k}"))?;
            let value = HeaderValue::from_str(v)
                .with_context(|| format!("invalid header value for {k}"))?;
            map.insert(name, value);
        }
        req = req.headers(map);
    }

    let resp = req.send().await.context("sending HTTP request")?;

    if let Some(code) = status_code {
        if resp.status().as_u16() != code {
            return Err(anyhow!("unexpected status {} (expected {})", resp.status(), code));
        }
    }

    let needs_body = expected_body_substring.is_some() || expected_body_regex.is_some();
    if needs_body {
        let body = resp.text().await.context("reading response body")?;

        if let Some(substr) = expected_body_substring {
            if !body.contains(substr) {
                return Err(anyhow!("response body missing substring"));
            }
        }

        if let Some(re) = expected_body_regex {
            let rx = Regex::new(re).context("compiling expected_body_regex")?;
            if !rx.is_match(&body) {
                return Err(anyhow!("response body regex did not match"));
            }
        }
    }

    Ok(())
}
