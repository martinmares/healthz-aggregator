use crate::config::{CheckConfig, CheckSpec};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue}, Client, Method};
use serde_json::Value;

fn json_path_lookup<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let p = path.trim();
    if p.is_empty() || p == "$" {
        return Some(root);
    }

    // accept "$" or "$.foo.bar[0]" ... small subset
    let mut cursor = root;
    let mut s = p;
    if s.starts_with("$.") {
        s = &s[2..];
    } else if s.starts_with('$') {
        s = &s[1..];
        if s.starts_with('.') {
            s = &s[1..];
        }
    }

    for seg in s.split('.') {
        if seg.is_empty() {
            continue;
        }

        // seg may be "a", "a[0]", "[0]" (rare)
        let mut rest = seg;

        // key part
        if !rest.starts_with('[') {
            let key_end = rest.find('[').unwrap_or_else(|| rest.len());
            let key = &rest[..key_end];
            cursor = match cursor {
                Value::Object(map) => map.get(key)?,
                _ => return None,
            };
            rest = &rest[key_end..];
        }

        // zero or more [idx] parts
        while rest.starts_with('[') {
            let close = rest.find(']')?;
            let idx_str = &rest[1..close];
            let idx: usize = idx_str.parse().ok()?;
            cursor = match cursor {
                Value::Array(arr) => arr.get(idx)?,
                _ => return None,
            };
            rest = &rest[close + 1..];
        }

        if !rest.is_empty() {
            // unsupported token (e.g. filters)
            return None;
        }
    }

    Some(cursor)
}

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let (
        url,
        method,
        headers,
        timeout,
        tls_verify,
        status_code,
        json_path,
        expected_value,
        expected_regex,
    ) = match &cfg.spec {
        CheckSpec::HttpJson {
            url,
            method,
            headers,
            timeout,
            tls_verify,
            status_code,
            json_path,
            expected_value,
            expected_regex,
        } => (
            url.as_str(),
            method.as_deref().unwrap_or("GET"),
            headers,
            *timeout,
            *tls_verify,
            *status_code,
            json_path.as_deref(),
            expected_value.as_deref(),
            expected_regex.as_deref(),
        ),
        _ => return Err(anyhow!("invalid check spec for http_json")),
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

    let json: Value = resp.json().await.context("parsing JSON body")?;

    if let Some(p) = json_path {
        let value = json_path_lookup(&json, p).ok_or_else(|| anyhow!("json_path not found"))?;

        if let Some(expect) = expected_value {
            let got = match value {
                Value::Null => "null".to_string(),
                Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            if got != expect {
                return Err(anyhow!("json_path value mismatch (got {got}, expected {expect})"));
            }
        }

        if let Some(re) = expected_regex {
            let rx = Regex::new(re).context("compiling expected_regex")?;
            let got = match value {
                Value::Null => "null".to_string(),
                Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            if !rx.is_match(&got) {
                return Err(anyhow!("json_path value regex did not match"));
            }
        }

        // If only json_path is set, presence is enough.
        return Ok(());
    }

    // No json_path => just "HTTP OK" according to status_code.
    Ok(())
}
