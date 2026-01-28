use crate::config::{CheckConfig, CheckSpec};
use anyhow::{anyhow, Context, Result};
use regex::Regex;

#[cfg(feature = "oracle")]
use oracle::Connection;

#[cfg(feature = "oracle")]
fn build_connect_string(
    connection_string: Option<&str>,
    host: Option<&str>,
    port: Option<u16>,
    service_name: Option<&str>,
    sid: Option<&str>,
) -> Result<String> {
    if let Some(cs) = connection_string {
        return Ok(cs.to_string());
    }

    let host = host.ok_or_else(|| anyhow!("oracle.host (or oracle.connection_string) is required"))?;
    let port = port.unwrap_or(1521);
    if let Some(svc) = service_name {
        return Ok(format!("{host}:{port}/{svc}"));
    }
    if let Some(sid) = sid {
        // Easy connect for SID is not universally supported, but this is the common format.
        return Ok(format!("{host}:{port}:{sid}"));
    }
    Err(anyhow!(
        "oracle.service_name or oracle.sid (or oracle.connection_string) is required"
    ))
}

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let (
        connection_string,
        host,
        port,
        service_name,
        sid,
        username,
        password,
        connect_timeout,
        query,
        expected_scalar,
        expected_contains,
        expected_regex,
    ) = match &cfg.spec {
        CheckSpec::Oracle {
            connection_string,
            host,
            port,
            service_name,
            sid,
            username,
            password,
            connect_timeout,
            query,
            expected_scalar,
            expected_contains,
            expected_regex,
        } => (
            connection_string.as_deref(),
            host.as_deref(),
            *port,
            service_name.as_deref(),
            sid.as_deref(),
            username.as_str(),
            password.as_deref().unwrap_or(""),
            *connect_timeout,
            query.as_str(),
            expected_scalar.as_deref(),
            expected_contains.as_deref(),
            expected_regex.as_deref(),
        ),
        _ => return Err(anyhow!("invalid check spec for oracle")),
    };

    #[cfg(not(feature = "oracle"))]
    {
        let _ = (
            connection_string,
            host,
            port,
            service_name,
            sid,
            username,
            password,
            connect_timeout,
            query,
            expected_scalar,
            expected_contains,
            expected_regex,
        );
        return Err(anyhow!(
            "oracle check is not enabled (compile with --features oracle)"
        ));
    }

    #[cfg(feature = "oracle")]
    {
        let timeout = connect_timeout.unwrap_or(std::time::Duration::from_secs(5));
        let connect_string = build_connect_string(connection_string, host, port, service_name, sid)?;
        let username = username.to_string();
        let password = password.to_string();
        let query = query.to_string();
        let expected_scalar = expected_scalar.map(|s| s.to_string());
        let expected_contains = expected_contains.map(|s| s.to_string());
        let expected_regex = expected_regex.map(|s| s.to_string());

        let handle = tokio::task::spawn_blocking(move || -> Result<String> {
            let conn = Connection::connect(&username, &password, &connect_string)
                .with_context(|| format!("oracle connect failed: {connect_string}"))?;

            let row = conn
                .query_row(&query, &[])
                .context("oracle query_row failed")?;

            let vals = row.sql_values();
            if vals.is_empty() {
                return Err(anyhow!("oracle query returned no columns"));
            }
            Ok(vals[0].to_string())
        });

        let got = tokio::time::timeout(timeout, handle)
            .await
            .map_err(|_| anyhow!("oracle connect/query timeout after {:?}", timeout))?
            .map_err(|e| anyhow!("oracle task join failed: {e}"))??;

        if let Some(exp) = expected_scalar
            && got != exp
        {
            return Err(anyhow!("expected scalar '{exp}', got '{got}'"));
        }

        if let Some(cont) = expected_contains
            && !got.contains(&cont)
        {
            return Err(anyhow!("result does not contain '{cont}'"));
        }

        if let Some(re) = expected_regex {
            let rx = Regex::new(&re).context("compiling expected_regex")?;
            if !rx.is_match(&got) {
                return Err(anyhow!("result regex did not match"));
            }
        }

        Ok(())
    }
}
