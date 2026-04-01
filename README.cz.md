# Healthz Aggregator

**Healthz Aggregator** je malá služba v Rustu, která periodicky spouští konfigurovatelné health checky, drží poslední výsledky v paměti a vystavuje:

- Kubernetes-friendly health endpointy (liveness/readiness)
- lehké webové UI
- Prometheus metriky

## TL;DR

**Healthz Aggregator** dává jedno místo, kde si rychle odpovíš na otázky:

- žije proces/služba?
- je celý aplikační stack zdravý?
- která konkrétní závislost padá?
- co přesně uvidí load balancer na health endpointu?

V praxi se hodí tam, kde jedna služba závisí zároveň na více věcech:

- HTTP/JSON endpointy
- TCP konektivita
- TLS certifikáty
- PostgreSQL / Oracle dotazy
- lokální soubory / konfigurační soubory

Místo ruční kontroly každé závislosti zvlášť dostaneš:

- jeden agregovaný pohled přes `/healthz`
- group-specific health pohledy pro LB nebo provozní řezy
- UI s detailem ke každému checku
- Prometheus metriky pro jednotlivé checky i groups

## Pojmenování (binárka vs HTTP endpointy)

Crate/binárka se jmenuje **`healthz-aggregator`** a HTTP endpointy schválně drží Kubernetes konvenci **`/healthz`**.

Tím pádem:

- název artefaktu popisuje, co to je (agregátor health checků)
- HTTP rozhraní používá konvence, které provozní nástroje obvykle očekávají

## Quick start

```bash
# build
cargo build --release

# spuštění (defaultně ./config.yaml)
./target/release/healthz-aggregator

# spuštění s explicitní cestou ke configu
./target/release/healthz-aggregator --config /path/to/config.yaml

# validace configu a konec
./target/release/healthz-aggregator --validate --config /path/to/config.yaml

# jednorázové spuštění všech checků a konec (vhodné pro CI/CD)
./target/release/healthz-aggregator --run-once --config /path/to/config.yaml

# jednorázové spuštění jednoho checku
./target/release/healthz-aggregator --check example-http --config /path/to/config.yaml

# jednorázové spuštění jedné group
./target/release/healthz-aggregator --group public-lb --config /path/to/config.yaml

# otevření UI v browseru
./target/release/healthz-aggregator --open

# otevření vlastní URL
./target/release/healthz-aggregator --open-url http://localhost:8998/ui

# nebo přes env
HEALTHZ_CONFIG=/path/to/config.yaml ./target/release/healthz-aggregator
```

Logování používá `tracing`. Úroveň lze řídit přes `RUST_LOG`, např.:

```bash
RUST_LOG=info ./target/release/healthz-aggregator
```

## Konfigurace

Konfigurace je v YAML a načítá se z:

- `--config /path/to/config.yaml` nebo `-c /path/to/config.yaml` (pokud je nastaveno)
- jinak `HEALTHZ_CONFIG` (pokud je nastaveno)
- `./config.yaml`

Časové hodnoty používají formát `humantime` (`30s`, `5m`, `2h`, ...).

Minimální příklad:

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

### Typy checků

Podporované typy checků (viz `src/config.rs`):

- `tcp`
- `http`
- `http_json` (umí vytáhnout hodnotu přes malý JSONPath subset jako `$.a.b[0].c`)
- `tls_cert` (expirace TLS certifikátu)
- `postgres` (SQL)
- `file` (text/json + exact/contains/regex)
- `oracle` (feature-gated)

Stručně, co pokrývají:

- `tcp` - základní síťová dostupnost `host:port`
- `http` - dostupnost HTTP endpointu, status code, match nad headers/body
- `http_json` - HTTP JSON endpoint s JSONPath extrakcí a validací hodnoty/regexu
- `tls_cert` - validita certifikátu a kontrola zbývající životnosti
- `postgres` - SQL konektivita + validace výsledku dotazu pro Postgres
- `oracle` - SQL konektivita + validace výsledku dotazu pro Oracle
- `file` - existence a obsah lokálního souboru pro text nebo JSON

Každý check může navíc nastavit:

- `critical: true|false` (default `true`)
  - pokud je `false`, failing check spadne do `WARN` a **neshodí** agregovaný endpoint
- `groups: [name, ...]`
  - každá referencovaná group musí být explicitně definovaná v top-level `groups:`
- `static_labels:` (per-check labely)
  - sloučí se s `metrics.static_labels` (per-check labely mají při kolizi přednost)

Top-level routing/output konfigurace:

- `groups:`
  - deklaruje pojmenované health groups
  - `default_profile` určuje, co vrací `GET /groups/{group}/healthz`
  - `profiles` je whitelist pro explicitní profilové endpointy
- `response_profiles:`
  - deklaruje znovupoužitelné OK/FAIL response kontrakty
  - groups mohou publikovat jeden nebo více whitelisted profilů
  - pokud group nemá `default_profile`, použije se built-in JSON response

### Doporučení pro návrh groups

- Preferuj groups jako logické health pohledy, např. `public-lb`, `dns-edge`, `certificates`, `local-files`
- Jeden check může patřit do více groups, pokud to má reálný provozní význam
- Vyhýbej se umbrella groupám jako `all`, `default`, `misc` nebo `demo-all`
- `All checks` nech jako globální pohled; pojmenované groups používej jen pro záměrné řezy nad stejným katalogem checků
- Pokud group začne vypadat jako náhodný pytel checků, rozděl ji nebo přejmenuj tak, aby byl její účel zřejmý

## HTTP endpointy

### Self health (liveness)

- `GET /healthz`
- `GET /healthz/self`

Pokud proces žije, vždy vrací `200 OK`.

### Aggregate health (readiness)

- `GET /healthz/aggregate`

Aliasy:

- `GET /healthz/aggregated`
- `GET /multi-healthz`
- `GET /multi-health`

Vrací JSON summary posledních výsledků checků.

- vrací `200 OK`, když je agregát v pořádku
- vrací `503 Service Unavailable`, když je agregát `FAILED`

### Group health

- `GET /groups/{group}/healthz`

Vrací agregovaný výsledek pro jednu pojmenovanou group. Pokud group definuje `default_profile`, použijí se jeho response body/content-type/status kódy; jinak se vrací built-in JSON response.

- `GET /groups/{group}/healthz/profiles/{profile}`

Vrací stejný agregovaný stav, ale vyrenderovaný přes explicitní whitelisted `profile`.

- vrací `404 Not Found`, když group neexistuje
- vrací `404 Not Found`, když profil není pro danou group na whitelistu

## CI / dry-run CLI

Binárka umí běžet i v one-shot režimu bez startu HTTP serveru.

- `--validate` - parse + validace configu, potom konec
- `--run-once` - spustí všechny checky jednou, vypíše aggregate summary, potom konec
- `--check <name>` - spustí jednou jeden pojmenovaný check, vypíše výsledek, potom konec
- `--group <name>` - spustí jednou checky z jedné group, vypíše aggregate group výsledek, potom konec
- `--output json` - místo textu vrací strukturovaný JSON pro výše uvedené one-shot režimy

Chování exit code:

- `0` - validace/check/group/global aggregate dopadly dobře
- `1` - validace selhala, vybraný check selhal nebo aggregate selhal

### Details (JSON)

- `GET /healthz/details` - všechny checky + uptime + timestamps
- `GET /healthz/details/{check_name}` - detail jednoho checku
- `GET /groups/{group}/healthz/details` - detail checků patřících do jedné group

### UI

- `GET /` - redirect na `/ui`
- `GET /ui` - HTML UI
- `GET /ui?group={group}` - HTML UI scoped na jednu group
- `GET /ui/api/snapshot` - JSON snapshot pro client-side partial refresh (bez full page reloadu)
- `GET /ui/api/snapshot?group={group}` - scoped snapshot pro jednu group

Statické UI assety:

- `GET /static/ui.js` (embedded v binárce)
- `GET /static/ui.css` (embedded v binárce)
- `GET /static/vendor/...` (self-hosted Tabler CSS + icons + fonts)

Existuje i filesystem fallback:

- `GET /static/*` (servírované z lokálního `./static` adresáře)

### Prometheus metriky

- `GET /metrics`

Check-level metriky zůstávají beze změny. Navíc se exportují i group-level metriky:

- `healthz_group_up{group="public-lb"}`
- `healthz_group_checks_total{group="public-lb"}`
- `healthz_group_checks_down{group="public-lb"}`
- `healthz_group_checks_warn{group="public-lb"}`

### Příklad odpovědi pro HW load balancer

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

To pak dává:

- `GET /groups/public-lb/healthz` -> `OK` nebo `FAIL`
- `GET /groups/public-lb/healthz/profiles/hw-lb-text` -> stejný explicitní kontrakt

## Kubernetes probes (příklad)

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

## Poznámky k UI

UI je postavené na Tabler/Bootstrap a podporuje:

- automatický partial refresh podle `global.refresh_interval`
- přepínání scope mezi `All checks` a jednou nakonfigurovanou group
- filtrování podle názvu checku (case-insensitive substring)
- status toggles (UP/WARN/DOWN); pokud není vybráno nic, nezobrazí se nic
- hover popovery pro `Error` a `Labels` (placement **left**)
- modální okno pro JSON detail jednotlivého checku (fetched z `/healthz/details/{check_name}`)
- `test profile` modal pro live ověření group health response přes whitelisted profiles
- scrollovatelný obsah tabulky, aby stránka zůstala kompaktní

## Volitelný Oracle check feature

Oracle check je schovaný za feature `oracle`:

```bash
cargo build --release --features oracle
```

Za běhu budeš navíc potřebovat Oracle client knihovny dostupné v prostředí.

## License

Projekt je licencovaný pod MIT License. Detaily viz [LICENSE](LICENSE).

## Autor

Martin Mareš
