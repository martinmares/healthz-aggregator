mod checks;
mod config;
mod http;
mod scheduler;
mod state;

use crate::{config::Config, http::metrics::Metrics, state::AppState};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Parser)]
#[command(name = "healthz-aggregator")]
struct Cli {
    /// Path to YAML config file
    #[arg(short, long, env = "HEALTHZ_CONFIG", value_name = "PATH")]
    config: Option<String>,

    /// Open browser at http://<bind>/ui
    #[arg(long)]
    open: bool,

    /// Open browser at the provided URL
    #[arg(long, value_name = "URL")]
    open_url: Option<String>,

    /// Validate config, print result, and exit
    #[arg(long, conflicts_with_all = ["run_once", "check", "group", "open", "open_url"])]
    validate: bool,

    /// Run selected checks once and exit
    #[arg(long, conflicts_with_all = ["validate", "open", "open_url"])]
    run_once: bool,

    /// Run one named check and exit
    #[arg(long, value_name = "NAME", conflicts_with_all = ["validate", "group", "open", "open_url"])]
    check: Option<String>,

    /// Run checks from one group and exit
    #[arg(long, value_name = "NAME", conflicts_with_all = ["validate", "check", "open", "open_url"])]
    group: Option<String>,

    /// Output format for CLI one-shot modes
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output: OutputFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Serialize)]
struct CliErrorResponse {
    status: &'static str,
    message: String,
}

#[derive(Serialize)]
struct ValidateResponse {
    status: &'static str,
    mode: &'static str,
}

#[derive(Serialize)]
struct SingleCheckResponse {
    status: &'static str,
    mode: &'static str,
    check: crate::state::CheckResult,
}

#[derive(Serialize)]
struct AggregateResponse<'a> {
    status: &'static str,
    mode: &'static str,
    scope: &'a str,
    name: Option<&'a str>,
    aggregate_ok: bool,
    summary: &'a crate::state::AggregateSummary,
    failed: &'a [crate::state::CheckResult],
    warn: &'a [crate::state::CheckResult],
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();

    let cli_mode = cli.validate || cli.run_once || cli.check.is_some() || cli.group.is_some();
    let cfg = match Config::load_from_path(cli.config.as_deref()) {
        Ok(cfg) => cfg,
        Err(err) if cli_mode && cli.output == OutputFormat::Json => {
            print_json(&CliErrorResponse {
                status: "error",
                message: err.to_string(),
            });
            std::process::exit(1);
        }
        Err(err) => return Err(err),
    };

    if cli.validate {
        match cli.output {
            OutputFormat::Text => println!("config OK"),
            OutputFormat::Json => print_json(&ValidateResponse {
                status: "ok",
                mode: "validate",
            }),
        }
        return Ok(());
    }

    let state = Arc::new(AppState::new(&cfg));

    if cli.run_once || cli.check.is_some() || cli.group.is_some() {
        let exit_code = match run_cli_once(state, &cfg, &cli).await {
            Ok(()) => 0,
            Err(err) => {
                if cli.output == OutputFormat::Json {
                    print_json(&CliErrorResponse {
                        status: "error",
                        message: err.to_string(),
                    });
                }
                1
            }
        };

        if exit_code == 0 {
            return Ok(());
        }
        std::process::exit(exit_code);
    }

    scheduler::spawn(
        state.clone(),
        cfg.global.refresh_interval,
        cfg.global.max_concurrency,
        cfg.global.default_timeout,
    );

    let metrics_cfg = cfg
        .metrics
        .clone()
        .unwrap_or(crate::config::MetricsConfig {
            namespace: None,
            name: None,
            static_labels: None,
        });

    let metrics = Arc::new(Metrics::new(&metrics_cfg, &cfg.checks));

    let app = http::router(state, metrics);
    let bind_addr = &cfg.server.bind;
    let listener = TcpListener::bind(bind_addr).await?;

    tracing::info!(bind_addr = %bind_addr, "healthz-aggregator HTTP server started");

    if cli.open || cli.open_url.is_some() {
        let url = cli.open_url.or_else(|| ui_url_from_bind(bind_addr));
        if let Some(url) = url {
            let _ = webbrowser::open(&url);
        }
    }

    axum::serve(listener, app).await?;

    Ok(())
}

fn ui_url_from_bind(bind_addr: &str) -> Option<String> {
    let mut parts = bind_addr.rsplitn(2, ':');
    let port = parts.next()?;
    let host = parts.next().unwrap_or("localhost");
    let host = match host {
        "0.0.0.0" | "127.0.0.1" | "::" | "[::]" => "localhost",
        other => other,
    };
    Some(format!("http://{host}:{port}/ui"))
}

async fn run_cli_once(state: Arc<AppState>, cfg: &Config, cli: &Cli) -> anyhow::Result<()> {
    let selected_checks = if let Some(check_name) = &cli.check {
        let check = cfg
            .checks
            .iter()
            .find(|check| check.name == *check_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown check '{check_name}'"))?;
        vec![check]
    } else if let Some(group_name) = &cli.group {
        if !cfg.groups.contains_key(group_name) {
            anyhow::bail!("unknown group '{group_name}'");
        }

        let checks: Vec<_> = cfg
            .checks
            .iter()
            .filter(|check| check.groups.iter().any(|group| group == group_name))
            .cloned()
            .collect();
        if checks.is_empty() {
            anyhow::bail!("group '{group_name}' does not contain any checks");
        }
        checks
    } else {
        cfg.checks.clone()
    };

    let semaphore = cfg
        .global
        .max_concurrency
        .map(|n| Arc::new(Semaphore::new(n)));

    scheduler::run_checks_once(
        state.clone(),
        selected_checks.clone(),
        semaphore,
        cfg.global.default_timeout,
    )
    .await;

    if let Some(check_name) = &cli.check {
        print_single_check_result(&state, check_name, cli.output).await?;
    } else if let Some(group_name) = &cli.group {
        print_group_result(&state, group_name, cli.output).await?;
    } else {
        print_global_result(&state, cli.output).await?;
    }

    Ok(())
}

async fn print_single_check_result(
    state: &AppState,
    check_name: &str,
    output: OutputFormat,
) -> anyhow::Result<()> {
    let result = state
        .get(check_name)
        .await
        .ok_or_else(|| anyhow::anyhow!("missing result for check '{check_name}'"))?;

    match output {
        OutputFormat::Text => {
            println!("check: {}", result.name);
            println!("status: {}", status_text(result.status));
            println!("critical: {}", result.critical);
            println!(
                "duration: {}",
                result
                    .duration
                    .map(humantime::format_duration)
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            if let Some(error) = &result.error {
                println!("error: {error}");
            }
        }
        OutputFormat::Json => print_json(&SingleCheckResponse {
            status: if matches!(result.status, crate::state::CheckStatus::Down) && result.critical {
                "failed"
            } else {
                "ok"
            },
            mode: "check",
            check: result.clone(),
        }),
    }

    if matches!(result.status, crate::state::CheckStatus::Down) && result.critical {
        anyhow::bail!("check '{check_name}' failed");
    }

    Ok(())
}

async fn print_group_result(
    state: &AppState,
    group_name: &str,
    output: OutputFormat,
) -> anyhow::Result<()> {
    let (ok, summary, failed, warn) = state
        .aggregate_snapshot_for_group(group_name)
        .await
        .ok_or_else(|| anyhow::anyhow!("group '{group_name}' not found"))?;

    match output {
        OutputFormat::Text => {
            println!("group: {group_name}");
            println!("aggregate: {}", if ok { "OK" } else { "FAILED" });
            println!(
                "summary: total={}, up={}, warn={}, down={}, critical_down={}",
                summary.total, summary.up, summary.warn, summary.down, summary.critical_down
            );
            print_problem_checks("failed", &failed);
            print_problem_checks("warn", &warn);
        }
        OutputFormat::Json => print_json(&AggregateResponse {
            status: if ok { "ok" } else { "failed" },
            mode: "group",
            scope: "group",
            name: Some(group_name),
            aggregate_ok: ok,
            summary: &summary,
            failed: &failed,
            warn: &warn,
        }),
    }

    if ok {
        return Ok(());
    }

    anyhow::bail!("group '{group_name}' aggregate failed")
}

async fn print_global_result(state: &AppState, output: OutputFormat) -> anyhow::Result<()> {
    let (ok, summary, failed, warn) = state.aggregate_snapshot().await;

    match output {
        OutputFormat::Text => {
            println!("aggregate: {}", if ok { "OK" } else { "FAILED" });
            println!(
                "summary: total={}, up={}, warn={}, down={}, critical_down={}",
                summary.total, summary.up, summary.warn, summary.down, summary.critical_down
            );
            print_problem_checks("failed", &failed);
            print_problem_checks("warn", &warn);
        }
        OutputFormat::Json => print_json(&AggregateResponse {
            status: if ok { "ok" } else { "failed" },
            mode: "run_once",
            scope: "all",
            name: None,
            aggregate_ok: ok,
            summary: &summary,
            failed: &failed,
            warn: &warn,
        }),
    }

    if ok {
        return Ok(());
    }

    anyhow::bail!("aggregate failed")
}

fn print_problem_checks(label: &str, checks: &[crate::state::CheckResult]) {
    if checks.is_empty() {
        return;
    }

    println!("{label}:");
    for check in checks {
        match &check.error {
            Some(error) => println!("  - {}: {}", check.name, error),
            None => println!("  - {}", check.name),
        }
    }
}

fn status_text(status: crate::state::CheckStatus) -> &'static str {
    match status {
        crate::state::CheckStatus::Up => "UP",
        crate::state::CheckStatus::Warn => "WARN",
        crate::state::CheckStatus::Down => "DOWN",
    }
}

fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("failed to render JSON output: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ui_url_from_bind;

    #[test]
    fn ui_url_from_bind_localhost() {
        assert_eq!(
            ui_url_from_bind("0.0.0.0:8080").as_deref(),
            Some("http://localhost:8080/ui")
        );
        assert_eq!(
            ui_url_from_bind("[::]:9000").as_deref(),
            Some("http://localhost:9000/ui")
        );
        assert_eq!(
            ui_url_from_bind("127.0.0.1:1234").as_deref(),
            Some("http://localhost:1234/ui")
        );
    }
}
