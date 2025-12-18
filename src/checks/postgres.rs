use super::tls_client;
use crate::config::{CheckConfig, CheckSpec};
use anyhow::{Context, Result, anyhow};
use regex::Regex;
use std::time::Duration;
use tokio_postgres::{Row, types::ToSql};

fn scalar_to_string(row: &Row) -> Result<String> {
    // Try common scalar shapes in order.
    if let Ok(v) = row.try_get::<usize, Option<String>>(0) {
        if let Some(v) = v {
            return Ok(v);
        }
    }

    if let Ok(v) = row.try_get::<usize, String>(0) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<usize, Option<i64>>(0) {
        if let Some(v) = v {
            return Ok(v.to_string());
        }
    }
    if let Ok(v) = row.try_get::<usize, i64>(0) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<i32>>(0) {
        if let Some(v) = v {
            return Ok(v.to_string());
        }
    }
    if let Ok(v) = row.try_get::<usize, i32>(0) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<i16>>(0) {
        if let Some(v) = v {
            return Ok(v.to_string());
        }
    }
    if let Ok(v) = row.try_get::<usize, i16>(0) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<f64>>(0) {
        if let Some(v) = v {
            return Ok(v.to_string());
        }
    }
    if let Ok(v) = row.try_get::<usize, f64>(0) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, Option<bool>>(0) {
        if let Some(v) = v {
            return Ok(v.to_string());
        }
    }
    if let Ok(v) = row.try_get::<usize, bool>(0) {
        return Ok(v.to_string());
    }

    Err(anyhow!(
        "unable to convert first column to string (try casting in SQL, e.g. SELECT value::text)"
    ))
}

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let (
        host,
        port,
        database,
        username,
        password,
        connect_timeout,
        tls_enabled,
        ignore_invalid_cert,
        query,
        expected_scalar,
        expected_contains,
        expected_regex,
    ) = match &cfg.spec {
        CheckSpec::Postgres {
            host,
            port,
            database,
            username,
            password,
            connect_timeout,
            tls,
            ignore_invalid_cert,
            query,
            expected_scalar,
            expected_contains,
            expected_regex,
        } => (
            host.clone(),
            port.unwrap_or(5432),
            database.clone(),
            username.clone(),
            password.clone(),
            connect_timeout.unwrap_or(Duration::from_secs(5)),
            tls.unwrap_or(false),
            ignore_invalid_cert.unwrap_or(false),
            query.clone(),
            expected_scalar.clone(),
            expected_contains.clone(),
            expected_regex.clone(),
        ),
        _ => return Err(anyhow!("invalid check spec for postgres")),
    };

    let mut pg_cfg = tokio_postgres::Config::new();
    pg_cfg.host(&host);
    pg_cfg.port(port);
    pg_cfg.dbname(&database);
    pg_cfg.user(&username);
    pg_cfg.connect_timeout(connect_timeout);
    if let Some(pw) = password {
        pg_cfg.password(pw);
    }

    // Connect, but don't try to store the typed connection (TLS stream differs from NoTls).
    let client = if tls_enabled {
        let tls_verify = !ignore_invalid_cert;
        let tls_cfg = tls_client::client_config(tls_verify)?;
        let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_cfg.as_ref().clone());

        let (client, connection) = tokio::time::timeout(connect_timeout, pg_cfg.connect(tls))
            .await
            .map_err(|_| anyhow!("postgres connect timeout"))?
            .context("postgres connect failed")?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::warn!(error = %e, "postgres connection task ended with error");
            }
        });

        client
    } else {
        let (client, connection) =
            tokio::time::timeout(connect_timeout, pg_cfg.connect(tokio_postgres::NoTls))
                .await
                .map_err(|_| anyhow!("postgres connect timeout"))?
                .context("postgres connect failed")?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::warn!(error = %e, "postgres connection task ended with error");
            }
        });

        client
    };

    // Query timeout: reuse connect_timeout for now (keeps config small & predictable).
    let query_timeout = connect_timeout;

    let params: &[&(dyn ToSql + Sync)] = &[];

    let row_opt = tokio::time::timeout(query_timeout, client.query_opt(&query, params))
        .await
        .map_err(|_| anyhow!("postgres query timeout"))?
        .context("postgres query failed")?;

    let row = row_opt.ok_or_else(|| anyhow!("postgres query returned no rows"))?;
    let value = scalar_to_string(&row)?;

    if let Some(expect) = expected_scalar {
        if value != expect {
            return Err(anyhow!(
                "postgres scalar mismatch (got {value:?}, expected {expect:?})"
            ));
        }
    }

    if let Some(substr) = expected_contains {
        if !value.contains(&substr) {
            return Err(anyhow!(
                "postgres scalar missing substring (got {value:?}, expected to contain {substr:?})"
            ));
        }
    }

    if let Some(re) = expected_regex {
        let rx = Regex::new(&re).context("compiling postgres expected_regex")?;
        if !rx.is_match(&value) {
            return Err(anyhow!(
                "postgres scalar regex did not match (got {value:?}, regex {re:?})"
            ));
        }
    }

    Ok(())
}
