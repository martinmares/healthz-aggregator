# Healthcheck Aggregator

**Healthcheck Aggregator** is a small Rust service that periodically runs a configurable set of health checks, keeps the latest results in memory, and exposes:

- Kubernetes-friendly health endpoints (for liveness/readiness)
- a lightweight web UI
- Prometheus metrics

It’s meant to be a pragmatic “single place to look” when your world contains more than one thing that can be on fire.

## Naming (binary vs HTTP endpoints)

The crate/binary is named **`healthcheck-aggregator`**, but the HTTP endpoints intentionally keep the Kubernetes convention of **`/healthz`**.

That way:

- the *artifact name* describes what it is (an aggregator of health checks), and
- the *HTTP surface* looks like what Kubernetes tooling expects.

## Quick start

```bash
# build
cargo build --release

# run (defaults to ./config.yaml)
./target/release/healthcheck-aggregator

# open the UI in a browser
./target/release/healthcheck-aggregator --open

# open a custom URL
./target/release/healthcheck-aggregator --open-url http://localhost:8998/ui

# or explicitly
HEALTHZ_CONFIG=/path/to/config.yaml ./target/release/healthcheck-aggregator
```

Logging uses `tracing`. Control verbosity with `RUST_LOG`, e.g.:

```bash
RUST_LOG=info ./target/release/healthcheck-aggregator
```

## Configuration

Configuration is YAML and is loaded from:

- `HEALTHZ_CONFIG` (if set), otherwise
- `./config.yaml`

Durations use the `humantime` format (`30s`, `5m`, `2h`, ...).

Minimal example:

```yaml
server:
  bind: 0.0.0.0:8998

global:
  refresh_interval: 30s
  # default_timeout: 10s
  # max_concurrency: 20

metrics:
  namespace: healthcheck
  name: aggregator
  static_labels:
    env: dev
    cluster: home

checks:
  - name: router-tcp
    critical: true
    type: tcp
    host: 10.0.0.1
    port: 22

  - name: example-http
    critical: true
    type: http
    url: https://example.com/health
    status_code: 200
```

### Check types

Supported check types (see `src/config.rs`):

- `tcp`
- `http`
- `http_json` (extract a value via a small JSONPath subset like `$.a.b[0].c`)
- `tls_cert` (TLS certificate expiry)
- `postgres` (SQL)
- `file` (text/json + exact/contains/regex)
- `oracle` (feature-gated)

Each check can also set:

- `critical: true|false` (default `true`)
  - when `false`, a failing check becomes `WARN` and **does not fail** the aggregate endpoint
- `static_labels:` (per-check labels)
  - merged with `metrics.static_labels` (check-level labels win on key collisions)

## HTTP endpoints

### Self health (liveness)

- `GET /healthz`
- `GET /healthz/self`

Always returns `200 OK` if the process is alive.

### Aggregate health (readiness)

- `GET /healthz/aggregate`

Aliases:

- `GET /healthz/aggregated`
- `GET /multi-healthz`
- `GET /multi-health`

Returns a JSON summary of the latest check results.

- returns `200 OK` when the aggregate is OK
- returns `503 Service Unavailable` when the aggregate is FAILED

### Details (JSON)

- `GET /healthz/details` – all checks + uptime + timestamps
- `GET /healthz/details/{check_name}` – details for a single check

### UI

- `GET /` – redirects to `/ui`
- `GET /ui` – HTML UI
- `GET /ui/api/snapshot` – JSON snapshot used for client-side partial refresh (no full-page reload)

Static UI assets:

- `GET /static/ui.js` (embedded in the binary)
- `GET /static/ui.css` (embedded in the binary)
- `GET /static/vendor/...` (self-hosted Tabler CSS + icons + fonts)

There is also a filesystem fallback:

- `GET /static/*` (served from the local `./static` directory)

### Prometheus metrics

- `GET /metrics`

## Kubernetes probes (example)

```yaml
livenessProbe:
  httpGet:
    path: /healthz
    port: 8998

readinessProbe:
  httpGet:
    path: /healthz/aggregate
    port: 8998
```

## UI notes

The UI is built with Tabler/Bootstrap and supports:

- automatic partial refresh based on `global.refresh_interval`
- filter by check name (case-insensitive substring)
- status toggles (UP/WARN/DOWN); if nothing is selected, nothing is shown
- popovers on hover for `Error` and `Labels` (placement **left**)
- a modal for per-check JSON details (fetched from `/healthz/details/{check_name}`)
- a scrollable table body to keep the page compact

## Optional: Oracle check feature

The Oracle check is gated behind the `oracle` feature:

```bash
cargo build --release --features oracle
```

At runtime you’ll also need Oracle client libraries available in the environment.

## Contributing

Issues and PRs are welcome. If you’re adding a new check type, please try to keep:

- config schema backwards-compatible (YAML is a public API), and
- error messages useful (they’re surfaced in UI popovers).

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Author

Martin Mareš
