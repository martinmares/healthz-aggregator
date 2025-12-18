use crate::config::{CheckConfig, CheckSpec};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde_json::Value;

use super::json_path;

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let (path, format, json_path_expr, expected_value, expected_contains, expected_regex) =
        match &cfg.spec {
            CheckSpec::File {
                path,
                format,
                json_path,
                expected_value,
                expected_contains,
                expected_regex,
            } => (
                path.as_str(),
                format.as_deref(),
                json_path.as_deref(),
                expected_value.as_deref(),
                expected_contains.as_deref(),
                expected_regex.as_deref(),
            ),
            _ => return Err(anyhow!("invalid check spec for file")),
        };

    // Existence check (fast path).
    tokio::fs::metadata(path)
        .await
        .with_context(|| format!("file does not exist or is not accessible: {path}"))?;

    let needs_content = json_path_expr.is_some()
        || expected_value.is_some()
        || expected_contains.is_some()
        || expected_regex.is_some()
        || matches!(format, Some("json"));

    if !needs_content {
        return Ok(());
    }

    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("reading file failed: {path}"))?;
    let text = String::from_utf8(bytes).context("file content is not valid UTF-8")?;

    let json_mode = matches!(format, Some("json")) || json_path_expr.is_some();
    let got = if json_mode {
        let json: Value = serde_json::from_str(&text).context("parsing file as JSON")?;

        if let Some(p) = json_path_expr {
            let v = json_path::lookup(&json, p)
                .ok_or_else(|| anyhow!("json_path not found in file"))?;
            value_to_string(v)
        } else {
            // Only JSON validity requested.
            json.to_string()
        }
    } else {
        text
    };

    if let Some(exp) = expected_value {
        if got != exp {
            return Err(anyhow!(
                "file value mismatch (got '{got}', expected '{exp}')"
            ));
        }
    }

    if let Some(cont) = expected_contains {
        if !got.contains(cont) {
            return Err(anyhow!("file value does not contain '{cont}'"));
        }
    }

    if let Some(re) = expected_regex {
        let rx = Regex::new(re).context("compiling expected_regex")?;
        if !rx.is_match(&got) {
            return Err(anyhow!("file value regex did not match"));
        }
    }

    // If only json_path was provided, presence was already validated.
    Ok(())
}
