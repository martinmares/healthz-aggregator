[*] Zaměnit std::sync::RwLock za tokio::sync::RwLock a upravit volání na async.
[*] Zajistit hard-timeout pro všechny checky (globální fallback), aby scheduler nemohl viset.
[*] Odstranit unwrap/expect v běžných cestách (HTTP handlers, metrics, scheduler) a nahradit bezpečným fallbackem + log.
[*] Zajistit stabilitu UI při výpadku CDN (self-host nebo SRI + fallback).
[*] Přidat základní testy pro kritické utility (sanitize_*, ui_url_from_bind, základní handlers).
