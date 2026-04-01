# Healthz Aggregator

For the Czech version, see [README.cz.md](README.cz.md).

**Healthz Aggregator** is a small Rust service that periodically runs a configurable set of health checks, keeps the latest results in memory, and exposes:

- Kubernetes-friendly health endpoints (for liveness/readiness)
- a lightweight web UI
- Prometheus metrics

It’s meant to be a pragmatic “single place to look” when your world contains more than one thing that can be on fire.

## TL;DR

**Healthz Aggregator** gives one place where they can quickly answer:

- is the service/process alive?
- is the whole application slice healthy?
- which concrete dependency is failing?
- what exactly would a load balancer see on the health endpoint?

In practice it helps when one service depends on multiple things at once:

- HTTP/JSON endpoints
- TCP reachability
- TLS certificates
- PostgreSQL / Oracle queries
- local files / config files

Instead of checking each dependency manually, gets:

- one aggregate `/healthz` view
- group-specific health views for LB or operational slices
- a UI with details for each check
- Prometheus metrics for both individual checks and groups

## Naming (binary vs HTTP endpoints)

The crate/binary is named **`healthz-aggregator`**, and the HTTP endpoints intentionally keep the Kubernetes convention of **`/healthz`**.

That way:

- the *artifact name* describes what it is (an aggregator of health checks), and
- the *HTTP surface* looks like what Kubernetes tooling expects.

## Quick start

```bash
# build
cargo build --release

# run (defaults to ./config.yaml)
./target/release/healthz-aggregator

# run with explicit config path
./target/release/healthz-aggregator --config /path/to/config.yaml

# validate config and exit
./target/release/healthz-aggregator --validate --config /path/to/config.yaml

# run all checks once and exit (useful for CI/CD)
./target/release/healthz-aggregator --run-once --config /path/to/config.yaml

# run one check once and exit
./target/release/healthz-aggregator --check example-http --config /path/to/config.yaml

# run one group once and exit
./target/release/healthz-aggregator --group public-lb --config /path/to/config.yaml

# open the UI in a browser
./target/release/healthz-aggregator --open

# open a custom URL
./target/release/healthz-aggregator --open-url http://localhost:8998/ui

# or explicitly
HEALTHZ_CONFIG=/path/to/config.yaml ./target/release/healthz-aggregator
```

Logging uses `tracing`. Control verbosity with `RUST_LOG`, e.g.:

```bash
RUST_LOG=info ./target/release/healthz-aggregator
```

## Configuration

Configuration is YAML and is loaded from:

- `--config /path/to/config.yaml` or `-c /path/to/config.yaml` (if set)
- otherwise `HEALTHZ_CONFIG` (if set)
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

response_profiles:
  default-json:
    ok:
      status_code: 200
      content_type: application/json
      body: '{"status":"ok"}'
    fail:
      status_code: 503
      content_type: application/json
      body: '{"status":"failed"}'

  hw-lb-text:
    ok:
      status_code: 200
      content_type: text/plain; charset=utf-8
      body: OK
    fail:
      status_code: 503
      content_type: text/plain; charset=utf-8
      body: FAIL

groups:
  public-lb:
    default_profile: hw-lb-text
    profiles: [hw-lb-text, default-json]
  internal-ui: {}

checks:
  - name: router-tcp
    critical: true
    groups: [public-lb]
    type: tcp
    host: 10.0.0.1
    port: 22

  - name: example-http
    critical: true
    groups: [public-lb, internal-ui]
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

Quick summary of what they cover:

- `tcp` - raw network reachability to `host:port`
- `http` - HTTP endpoint availability, response code, headers/body matching
- `http_json` - HTTP JSON endpoint with JSONPath extraction and value/regex assertions
- `tls_cert` - certificate validity and remaining lifetime checks
- `postgres` - SQL connectivity + query result validation
- `oracle` - SQL connectivity + query result validation for Oracle
- `file` - local file existence/content checks for text or JSON

Each check can also set:

- `critical: true|false` (default `true`)
  - when `false`, a failing check becomes `WARN` and **does not fail** the aggregate endpoint
- `groups: [name, ...]`
  - each referenced group must be explicitly defined in top-level `groups:`
- `static_labels:` (per-check labels)
  - merged with `metrics.static_labels` (check-level labels win on key collisions)

Top-level routing/output config:

- `groups:`
  - declares named health groups
  - `default_profile` controls what `GET /groups/{group}/healthz` returns
  - `profiles` is a whitelist for explicit profile endpoints
- `response_profiles:`
  - declares reusable OK/FAIL response contracts
  - groups can expose one or more whitelisted profiles
  - if a group has no `default_profile`, the built-in JSON response is used

### Group design guidelines

- Prefer groups as logical health views such as `public-lb`, `dns-edge`, `certificates`, `local-files`
- A single check may belong to more than one group when that overlap has a real operational meaning
- Avoid umbrella groups such as `all`, `default`, `misc`, or `demo-all`
- Keep `All checks` as the global view; use named groups only for intentional slices of the same check catalog
- If a group starts looking like a random bag of checks, split it or rename it until its purpose is obvious

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

### Group health

- `GET /groups/{group}/healthz`

Returns the aggregate result for a single named group. If the group defines a `default_profile`, its response body/content-type/status codes are used; otherwise the built-in JSON response is returned.

- `GET /groups/{group}/healthz/profiles/{profile}`

Returns the same aggregate state, but rendered through an explicit whitelisted `profile`.

- returns `404 Not Found` when the group does not exist
- returns `404 Not Found` when the profile is not whitelisted for the group

## CI / dry-run CLI

The binary can also run in one-shot mode without starting the HTTP server.

- `--validate` - parse + validate config, then exit
- `--run-once` - run all checks once, print aggregate summary, then exit
- `--check <name>` - run one named check once, print its result, then exit
- `--group <name>` - run checks from one group once, print group aggregate, then exit
- `--output json` - emit structured JSON instead of text for the one-shot modes above

Exit code behavior:

- `0` - validation/check/group/global aggregate succeeded
- `1` - validation failed, selected check failed, or aggregate failed

### Details (JSON)

- `GET /healthz/details` - all checks + uptime + timestamps
- `GET /healthz/details/{check_name}` - details for a single check
- `GET /groups/{group}/healthz/details` - details for checks that belong to a single group

### UI

- `GET /` - redirects to `/ui`
- `GET /ui` - HTML UI
- `GET /ui?group={group}` - HTML UI scoped to one group
- `GET /ui/api/snapshot` - JSON snapshot used for client-side partial refresh (no full-page reload)
- `GET /ui/api/snapshot?group={group}` - scoped snapshot for one group

Static UI assets:

- `GET /static/ui.js` (embedded in the binary)
- `GET /static/ui.css` (embedded in the binary)
- `GET /static/vendor/...` (self-hosted Tabler CSS + icons + fonts)

There is also a filesystem fallback:

- `GET /static/*` (served from the local `./static` directory)

### Prometheus metrics

- `GET /metrics`

Check-level metrics stay unchanged. Group-level metrics are also exported:

- `healthz_group_up{group="public-lb"}`
- `healthz_group_checks_total{group="public-lb"}`
- `healthz_group_checks_down{group="public-lb"}`
- `healthz_group_checks_warn{group="public-lb"}`

### HW load balancer response example

```yaml
response_profiles:
  hw-lb-text:
    ok:
      status_code: 200
      content_type: text/plain; charset=utf-8
      body: OK
    fail:
      status_code: 503
      content_type: text/plain; charset=utf-8
      body: FAIL

groups:
  public-lb:
    default_profile: hw-lb-text
    profiles: [hw-lb-text]
```

That yields:

- `GET /groups/public-lb/healthz` -> `OK` or `FAIL`
- `GET /groups/public-lb/healthz/profiles/hw-lb-text` -> the same explicit contract

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
- scope switching between `All checks` and a single configured group
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

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Author

Martin Mareš
