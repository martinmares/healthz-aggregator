use crate::config::{CheckConfig, CheckSpec};
use anyhow::{Context, Result, anyhow};
use regex::Regex;
use std::time::Duration;
use tokio_postgres::{NoTls, Row, types::ToSql};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::tls_client;

fn scalar_to_string(row: &Row) -> Result<String> {
    if let Ok(v) = row.try_get::<usize, String>(0) {
        return Ok(v);
    }
    if let Ok(v) = row.try_get::<usize, &str>(0) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, i64>(0) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, i32>(0) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, f64>(0) {
        return Ok(v.to_string());
    }
    if let Ok(v) = row.try_get::<usize, bool>(0) {
        return Ok(v.to_string());
    }
    Err(anyhow!("unsupported scalar type in first column"))
}

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let (
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
            host.as_str(),
            port.unwrap_or(5432),
            database.as_str(),
            username.as_str(),
            password.as_deref(),
            connect_timeout.unwrap_or(Duration::from_secs(5)),
            tls.unwrap_or(false),
            *ignore_invalid_cert,
            query.as_str(),
            expected_scalar.as_deref(),
            expected_contains.as_deref(),
            expected_regex.as_deref(),
        ),
        _ => return Err(anyhow!("invalid check spec for postgres")),
    };

    let mut pgcfg = tokio_postgres::Config::new();
    pgcfg.host(host);
    pgcfg.port(port);
    pgcfg.dbname(database);
    pgcfg.user(username);
    if let Some(pw) = password {
        pgcfg.password(pw);
    }

    let client = if tls {
        let tls_verify = !ignore_invalid_cert.unwrap_or(false);
        let tls_cfg = tls_client::client_config(tls_verify)?;
        let tls = MakeRustlsConnect::new(tls_cfg.as_ref().clone());

        let (client, connection) = tokio::time::timeout(connect_timeout, pgcfg.connect(tls))
            .await
            .map_err(|_| anyhow!("postgres connect timeout after {:?}", connect_timeout))?
            .context("postgres connect failed")?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::warn!(error = %e, "postgres connection error");
            }
        });

        client
    } else {
        let (client, connection) = tokio::time::timeout(connect_timeout, pgcfg.connect(NoTls))
            .await
            .map_err(|_| anyhow!("postgres connect timeout after {:?}", connect_timeout))?
            .context("postgres connect failed")?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::warn!(error = %e, "postgres connection error");
            }
        });

        client
    };

    let params: &[&(dyn ToSql + Sync)] = &[];
    let row_opt = tokio::time::timeout(connect_timeout, client.query_opt(query, params))
        .await
        .map_err(|_| anyhow!("postgres query timeout after {:?}", connect_timeout))?
        .context("postgres query failed")?;

    let Some(row) = row_opt else {
        return Err(anyhow!("postgres query returned no rows"));
    };

    let got = scalar_to_string(&row)?;

    if let Some(exp) = expected_scalar
        && got != exp
    {
        return Err(anyhow!(
            "postgres scalar mismatch (got '{got}', expected '{exp}')"
        ));
    }

    if let Some(cont) = expected_contains
        && !got.contains(cont)
    {
        return Err(anyhow!(
            "postgres scalar does not contain '{cont}' (got '{got}')"
        ));
    }

    if let Some(re) = expected_regex {
        let rx = Regex::new(re).context("compiling expected_regex")?;
        if !rx.is_match(&got) {
            return Err(anyhow!(
                "postgres scalar regex did not match (got '{got}', re '{re}')"
            ));
        }
    }

    Ok(())
}
