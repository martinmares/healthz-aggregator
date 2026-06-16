# Example Packs

This directory contains small, opinionated example configurations for common use cases.

Available packs:

- `loadbalancer-text.yaml`
  - plain text `OK` / `FAIL` response for a hardware or simple HTTP load balancer
- `loadbalancer-json.yaml`
  - minimal JSON response for a load balancer, proxy, or gateway
- `tls-and-certificates.yaml`
  - certificate validity and expiry checks
- `files-and-config.yaml`
  - local file / config validation for sidecar or host-level checks
- `public-demo.yaml`
  - broader public demo showing groups, profiles, history, debounce, and multiple check types

Typical usage:

```bash
./target/release/healthz-aggregator --validate -c examples/loadbalancer-text.yaml
./target/release/healthz-aggregator --run-once -c examples/loadbalancer-text.yaml
./target/release/healthz-aggregator -c examples/loadbalancer-text.yaml --open
```

Notes:

- the `files-and-config.yaml` pack expects `examples/demo-app.conf` and `examples/demo-app.json`
- the `public-demo.yaml` pack uses only public endpoints and local demo files shipped in this repo
