use crate::config::{CheckConfig, CheckSpec};
use anyhow::{anyhow, Result};
use tokio::net::TcpStream;

pub async fn run(cfg: &CheckConfig) -> Result<()> {
    let (host, port, timeout) = match &cfg.spec {
        CheckSpec::Tcp { host, port, timeout } => (host, *port, *timeout),
        _ => return Err(anyhow!("invalid check spec for tcp")),
    };

    let addr = format!("{}:{}", host, port);
    let connect = TcpStream::connect(addr);

    let default_to = std::time::Duration::from_secs(3);
    let to = timeout.unwrap_or(default_to);

    match tokio::time::timeout(to, connect).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(anyhow!("tcp connect failed: {}", e)),
        Err(_) => Err(anyhow!("tcp connect timed out after {:?}", to)),
    }
}
