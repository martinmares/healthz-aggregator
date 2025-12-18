use crate::config::{CheckConfig, CheckSpec};
use anyhow::{Context, Result, anyhow};
use rustls::pki_types::ServerName;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use url::Url;
use x509_parser::prelude::parse_x509_certificate;

use super::tls_client;

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let (host, port, sni_override, timeout, tls_verify, min_days_remaining) = match &cfg.spec {
        CheckSpec::TlsCert {
            host,
            port,
            url,
            sni,
            timeout,
            tls_verify,
            min_days_remaining,
        } => {
            // Resolve host/port from either host+port or URL
            let (h, p) = if let Some(u) = url {
                let parsed = Url::parse(u).context("parsing tls_cert.url")?;
                let h = parsed
                    .host_str()
                    .ok_or_else(|| anyhow!("tls_cert.url missing host"))?
                    .to_string();
                let p = parsed.port_or_known_default().unwrap_or(443);
                (h, p)
            } else {
                let h = host
                    .as_ref()
                    .ok_or_else(|| anyhow!("tls_cert requires either url or host"))?
                    .to_string();
                let p = port.unwrap_or(443);
                (h, p)
            };

            (
                h,
                p,
                sni.clone(),
                timeout.unwrap_or(Duration::from_secs(3)),
                tls_verify.unwrap_or(true),
                *min_days_remaining,
            )
        }
        _ => return Err(anyhow!("invalid check spec for tls_cert")),
    };

    let sni_name = sni_override.unwrap_or_else(|| host.clone());

    let addr = format!("{}:{}", host, port);
    let tcp = tokio::time::timeout(timeout, TcpStream::connect(&addr))
        .await
        .map_err(|_| anyhow!("tcp connect timed out after {:?}", timeout))?
        .with_context(|| format!("tcp connect failed: {addr}"))?;

    let tls_cfg = tls_client::client_config(tls_verify)?;
    let connector = TlsConnector::from(tls_cfg);

    let server_name: ServerName<'static> = ServerName::try_from(sni_name.clone())
        .map_err(|_| anyhow!("invalid SNI/server name: {sni_name}"))?;

    tracing::info!(server_name = ?server_name, sni_name = ?sni_name, timeout = ?timeout, addr = ?addr, "tls check");

    let tls_stream = tokio::time::timeout(timeout, connector.connect(server_name, tcp))
        .await
        .map_err(|_| anyhow!("tls handshake timed out after {:?}", timeout))?
        .context("tls handshake failed")?;

    // rustls stores peer certificates on the session.
    let certs = tls_stream
        .get_ref()
        .1
        .peer_certificates()
        .ok_or_else(|| anyhow!("no peer certificates presented"))?;

    let leaf = certs
        .first()
        .ok_or_else(|| anyhow!("empty peer certificate chain"))?;

    let (_rem, cert) = parse_x509_certificate(leaf.as_ref()).context("parsing leaf certificate")?;

    let not_after = cert.validity().not_after.to_datetime();

    let now = OffsetDateTime::now_utc();

    if not_after <= now {
        return Err(anyhow!("certificate already expired at {not_after}"));
    }

    if let Some(min_days) = min_days_remaining {
        let remaining = not_after - now;
        let remaining_days = remaining.whole_seconds() as f64 / 86_400.0;

        if remaining_days < min_days {
            return Err(anyhow!(
                "certificate expires too soon: {:.2} days remaining (min {:.2}), expires at {}",
                remaining_days,
                min_days,
                not_after
            ));
        }
    }

    Ok(())
}
