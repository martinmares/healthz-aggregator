[*] Zaměnit std::sync::RwLock za tokio::sync::RwLock a upravit volání na async.
[*] Zajistit hard-timeout pro všechny checky (globální fallback), aby scheduler nemohl viset.
[*] Odstranit unwrap/expect v běžných cestách (HTTP handlers, metrics, scheduler) a nahradit bezpečným fallbackem + log.
[*] Zajistit stabilitu UI při výpadku CDN (self-host nebo SRI + fallback).
[*] Přidat základní testy pro kritické utility (sanitize_*, ui_url_from_bind, základní handlers).
[*] Přidat explicitní definici `groups` a `response_profiles` do config schématu.
[*] Přidat group-aware endpointy `/groups/{group}/healthz` a `/groups/{group}/healthz/details`.
[*] Zachovat kompatibilitu stávajících globálních aggregate endpointů.
[*] Zdokumentovat groups/response profiles a otestovat validaci konfigurace.
[*] Rozšířit UI o přepínání mezi globálním pohledem a jednotlivými groups.
[*] Napojit UI snapshot endpoint na selected group bez rozbití default `/ui`.
[*] Umožnit `groups.<name>.default_profile` + whitelist `profiles[]` pro víc výstupních kontraktů nad stejnou group.
[*] Přidat endpoint `/groups/{group}/healthz/profiles/{profile}` pro explicitní profilovou odpověď.
[*] Rozšířit UI o `test profile` modal pro ověření live LB response nad aktivní group.
[*] Přidat group-level Prometheus metriky vedle stávajících check-level metrik.
[*] Přidat one-shot CLI režim pro `--validate`, `--run-once`, `--check` a `--group`, vhodný pro CI/CD.
[ ] Přidat state history pro posledních N výsledků checku jako základ pro debounce, UI trendy a notifikace.
[ ] Přidat debounce / threshold policy nad historií stavů.
[ ] Přidat UI trendy nad historií stavů.
[ ] Připravit export/import example packs pro běžné scénáře.
[ ] Přidat notifications při změně stavu.
[*] Doplnit README o TL;DR pro L2 a stručný přehled podporovaných checků.
