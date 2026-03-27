// Theme menu (Tabler / Bootstrap 5.3)
(function () {
  const key = "healthz-aggregator-ui-theme";
  const root = document.documentElement;
  const toggle = document.getElementById("theme-toggle");
  const menu = document.getElementById("theme-menu");
  const buttons = document.querySelectorAll("[data-theme]");

  function applyTheme(theme) {
    if (theme === "auto") {
      const prefersDark =
        window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: dark)").matches;
      root.setAttribute("data-bs-theme", prefersDark ? "dark" : "light");
    } else {
      root.setAttribute("data-bs-theme", theme);
    }
  }

  const saved = localStorage.getItem(key) || "auto";
  applyTheme(saved);

  if (toggle && menu) {
    toggle.addEventListener("click", (e) => {
      e.preventDefault();
      const isVisible = menu.style.display === "block";
      menu.style.display = isVisible ? "none" : "block";
    });

    document.addEventListener("click", (e) => {
      if (!toggle.contains(e.target) && !menu.contains(e.target)) {
        menu.style.display = "none";
      }
    });
  }

  buttons.forEach((button) => {
    button.addEventListener("click", (e) => {
      e.preventDefault();
      const theme = button.getAttribute("data-theme") || "auto";
      localStorage.setItem(key, theme);
      applyTheme(theme);
      if (menu) menu.style.display = "none";
    });
  });

  if (window.matchMedia) {
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      if ((localStorage.getItem(key) || "auto") === "auto") {
        applyTheme("auto");
      }
    });
  }
})();

// Helpers shared in this file.
const HealthzUi = (function () {
  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/\"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function formatLocalDateTime(value) {
    const d = value instanceof Date ? value : new Date(value);
    if (isNaN(d.getTime())) return null;
    return d.toLocaleString(undefined, {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  // Convert RFC3339 timestamps (UTC) to browser-local time.
  function formatLocalTimes(root) {
    const scope = root || document;
    const els = scope.querySelectorAll("time.js-localtime[datetime]");
    els.forEach((el) => {
      const dt = el.getAttribute("datetime");
      if (!dt) return;
      const formatted = formatLocalDateTime(dt);
      if (!formatted) return;
      el.textContent = formatted;
    });
  }

  function formatLastLocal(root) {
    const scope = root || document;
    const els = scope.querySelectorAll(".js-last-local[data-datetime]");
    els.forEach((el) => {
      const dt = el.getAttribute("data-datetime");
      if (!dt) return;
      const formatted = formatLocalDateTime(dt);
      if (!formatted) return;
      el.textContent = formatted;
    });
  }

  function timeAgoShort(nowMs, thenMs) {
    const diffMs = Math.max(0, nowMs - thenMs);
    const s = Math.floor(diffMs / 1000);
    if (s < 60) return `${s}s ago`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    const d = Math.floor(h / 24);
    if (d < 14) return `${d}d ago`;
    const w = Math.floor(d / 7);
    return `${w}w ago`;
  }

  // Update only the "(4m ago)" part; keep the ISO timestamp visible.
  function formatLastAgo(root, nowMs) {
    const scope = root || document;
    const now = typeof nowMs === "number" ? nowMs : Date.now();
    const els = scope.querySelectorAll(".js-last-ago[data-datetime]");
    els.forEach((el) => {
      const dt = el.getAttribute("data-datetime");
      if (!dt) return;
      const d = new Date(dt);
      if (isNaN(d.getTime())) return;
      el.textContent = `(${timeAgoShort(now, d.getTime())})`;
    });
  }

  function disposePopovers(root) {
    const scope = root || document;
    if (!window.bootstrap || !window.bootstrap.Popover) return;
    scope.querySelectorAll('[data-bs-toggle="popover"]').forEach((el) => {
      try {
        const existing = window.bootstrap.Popover.getInstance(el);
        if (existing) existing.dispose();
      } catch (_) {
        // ignore
      }
    });
  }

  function initPopovers(root) {
    const scope = root || document;
    if (!window.bootstrap || !window.bootstrap.Popover) return;

    scope.querySelectorAll('[data-bs-toggle="popover"]').forEach((el) => {
      try {
        const existing = window.bootstrap.Popover.getInstance(el);
        if (existing) existing.dispose();
        // We already escape user-provided strings before putting them into HTML,
        // so we can safely disable Bootstrap's sanitizer (it can strip our <div>/<br> layout).
        new window.bootstrap.Popover(el, { sanitize: false });
      } catch (_) {
        // ignore
      }
    });
  }

  function popoverHtmlForError(errorText) {
    const text = String(errorText || "");
    const safe = escapeHtml(text).replace(/\r?\n/g, "<br>");
    // Use single quotes inside: this HTML is later placed into data-bs-content.
    return `<div class='hc-popover-lines'>${safe || "—"}</div>`;
  }

  function popoverHtmlForLabels(labels) {
    const arr = Array.isArray(labels) ? labels : [];
    if (arr.length === 0) return `<div class='hc-popover-lines'>—</div>`;

    const lines = arr
      .filter((x) => x && String(x).trim() !== "")
      .map((x) => `<div>- ${escapeHtml(String(x))}</div>`)
      .join("");
    return `<div class='hc-popover-lines'>${lines}</div>`;
  }

  return {
    escapeHtml,
    formatLocalDateTime,
    formatLocalTimes,
    formatLastLocal,
    formatLastAgo,
    timeAgoShort,
    initPopovers,
    disposePopovers,
    popoverHtmlForError,
    popoverHtmlForLabels,
  };
})();

// Modal check details.
(function () {
  async function copyText(text) {
    if (!text) return;
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.left = "-9999px";
    ta.style.top = "-9999px";
    document.body.appendChild(ta);
    ta.select();
    try {
      document.execCommand("copy");
    } finally {
      document.body.removeChild(ta);
    }
  }

  async function fetchCheckDetails(name) {
    const resp = await fetch(`/healthz/details/${encodeURIComponent(name)}`, {
      headers: { Accept: "application/json" },
      cache: "no-store",
    });
    if (resp.status === 404) {
      return { check: null };
    }
    if (!resp.ok) {
      throw new Error(`HTTP ${resp.status} fetching /healthz/details/${name}`);
    }
    const check = await resp.json();
    return { check };
  }

  function setModal(title, text, source) {
    const t = document.getElementById("check-detail-modal-title");
    const pre = document.getElementById("check-detail-modal-json");
    const src = document.getElementById("check-detail-modal-source");
    if (t) t.textContent = title;
    if (pre) pre.textContent = text;
    if (src && source) src.textContent = source;
  }

  document.addEventListener("click", (evt) => {
    const copyBtn = evt.target && evt.target.closest
      ? evt.target.closest(".hc-copy-btn")
      : null;
    if (copyBtn) {
      evt.preventDefault();
      evt.stopPropagation();
      const text = copyBtn.getAttribute("data-copy-text") || "";
      (async () => {
        try {
          await copyText(text);
          copyBtn.classList.add("active");
          setTimeout(() => copyBtn.classList.remove("active"), 500);
        } catch (_) {
          // ignore
        }
      })();
      return;
    }

    const btn = evt.target && evt.target.closest
      ? evt.target.closest(".btn-check-detail")
      : null;
    if (!btn) return;

    const name = btn.getAttribute("data-check-name") || "";
    const source = `/healthz/details/${encodeURIComponent(name)}`;
    setModal(name ? `Check: ${name}` : "Check details", "Loading…", source);

    (async () => {
      try {
        const { check } = await fetchCheckDetails(name);
        if (!check) {
          setModal(`Check: ${name}`, `Not found (${source}).`, source);
          return;
        }
        setModal(`Check: ${name}`, JSON.stringify(check, null, 2), source);
      } catch (e) {
        setModal(
          `Check: ${name}`,
          `Failed to load details: ${e && e.message ? e.message : e}`,
          source
        );
      }
    })();
  });
})();

// Modal response profile tester.
(function () {
  let activeRequestId = 0;

  async function fetchProfileResponse(href) {
    const resp = await fetch(href, {
      headers: { Accept: "*/*" },
      cache: "no-store",
    });
    return {
      status: resp.status,
      contentType: resp.headers.get("content-type") || "—",
      body: await resp.text(),
    };
  }

  function setProfileModalState({ href, status, contentType, body }) {
    const url = document.getElementById("profile-test-url");
    const source = document.getElementById("profile-test-source");
    const statusEl = document.getElementById("profile-test-status");
    const contentTypeEl = document.getElementById("profile-test-content-type");
    const bodyEl = document.getElementById("profile-test-body");

    if (url) {
      url.textContent = href || "—";
      if (href) {
        url.setAttribute("href", href);
      } else {
        url.removeAttribute("href");
      }
    }
    if (source) source.textContent = href || "—";
    if (statusEl) statusEl.textContent = status || "—";
    if (contentTypeEl) contentTypeEl.textContent = contentType || "—";
    if (bodyEl) bodyEl.textContent = body || "";
  }

  async function runProfileFetch() {
    const root = document.getElementById("root");
    const group = root && root.dataset ? root.dataset.activeGroup || "" : "";
    const select = document.getElementById("profile-test-select");
    if (!group || !select || select.options.length === 0) {
      setProfileModalState({
        href: "",
        status: "—",
        contentType: "—",
        body: "Select a concrete group to test profile responses.",
      });
      return;
    }

    const href = (function () {
      const value = select.value || "__default__";
      if (value === "__default__") {
        return `/groups/${encodeURIComponent(group)}/healthz`;
      }
      return `/groups/${encodeURIComponent(group)}/healthz/profiles/${encodeURIComponent(value)}`;
    })();

    const requestId = ++activeRequestId;
    setProfileModalState({
      href,
      status: "Loading…",
      contentType: "Loading…",
      body: "Fetching live response…",
    });

    try {
      const result = await fetchProfileResponse(href);
      if (requestId !== activeRequestId) return;
      setProfileModalState({
        href,
        status: String(result.status),
        contentType: result.contentType,
        body: result.body || "",
      });
    } catch (e) {
      if (requestId !== activeRequestId) return;
      setProfileModalState({
        href,
        status: "request failed",
        contentType: "—",
        body: `Failed to fetch profile response: ${e && e.message ? e.message : e}`,
      });
    }
  }

  document.addEventListener("DOMContentLoaded", () => {
    const modal = document.getElementById("profile-test-modal");
    const select = document.getElementById("profile-test-select");
    if (!modal || !select) return;

    modal.addEventListener("shown.bs.modal", () => {
      runProfileFetch();
    });

    select.addEventListener("change", () => {
      runProfileFetch();
    });
  });
})();

// UI refresh (no HTMX; patch DOM in-place).
(function () {
  let inFlight = false;
  let refreshQueued = false;
  let lastServerNowMs = null;

  // Client-side filter for the checks table.
  // - case-insensitive
  // - substring match against check name
  let filterQuery = "";

  // Status filter: show UP/WARN/DOWN depending on checkboxes.
  // Default: all enabled.
  let filterShowUp = true;
  let filterShowWarn = true;
  let filterShowDown = true;

  function getRoot() {
    return document.getElementById("root");
  }

  function getActiveGroup() {
    const root = getRoot();
    return root && root.dataset ? root.dataset.activeGroup || "" : "";
  }

  function setActiveGroup(group) {
    const root = getRoot();
    if (!root || !root.dataset) return;

    const normalized = String(group || "").trim();
    root.dataset.activeGroup = normalized;
    root.dataset.snapshotHref = normalized
      ? `/ui/api/snapshot?group=${encodeURIComponent(normalized)}`
      : "/ui/api/snapshot";
    root.dataset.detailsHref = normalized
      ? `/groups/${encodeURIComponent(normalized)}/healthz/details`
      : "/healthz/details";
    root.dataset.scopeHealthHref = normalized
      ? `/groups/${encodeURIComponent(normalized)}/healthz`
      : "/healthz/aggregate";
  }

  function currentSnapshotHref() {
    const root = getRoot();
    return root && root.dataset && root.dataset.snapshotHref
      ? root.dataset.snapshotHref
      : "/ui/api/snapshot";
  }

  function currentUiHref() {
    const group = getActiveGroup();
    return group ? `/ui?group=${encodeURIComponent(group)}` : "/ui";
  }

  function resolveProfileHref(group, profile) {
    const normalizedGroup = String(group || "").trim();
    if (!normalizedGroup) return "";
    if (!profile || profile === "__default__") {
      return `/groups/${encodeURIComponent(normalizedGroup)}/healthz`;
    }
    return `/groups/${encodeURIComponent(normalizedGroup)}/healthz/profiles/${encodeURIComponent(profile)}`;
  }

  function replaceSelectOptions(select, options, preferredValue) {
    if (!select) return;
    const items = Array.isArray(options) ? options : [];
    const wanted = typeof preferredValue === "string" ? preferredValue : "";
    select.innerHTML = "";
    items.forEach((item) => {
      const option = document.createElement("option");
      option.value = item && item.value ? item.value : "";
      option.textContent = item && item.label ? item.label : option.value;
      option.selected = option.value === wanted || (!wanted && !!(item && item.selected));
      select.appendChild(option);
    });
    if (wanted && !items.some((item) => item && item.value === wanted) && select.options.length > 0) {
      select.selectedIndex = 0;
    }
  }

  function updateProfileUi(data) {
    const hasProfiles = !!(data && data.has_profile_testing);
    const trigger = document.getElementById("scope-profile-trigger");
    const separator = document.getElementById("scope-profile-separator");
    const wrap = document.getElementById("scope-default-profile-wrap");
    const defaultProfile = document.getElementById("scope-default-profile");
    const defaultLink = document.getElementById("scope-default-profile-link");
    const select = document.getElementById("profile-test-select");
    const title = document.getElementById("profile-test-modal-title");
    const body = document.getElementById("profile-test-body");
    const status = document.getElementById("profile-test-status");
    const contentType = document.getElementById("profile-test-content-type");
    const source = document.getElementById("profile-test-source");
    const url = document.getElementById("profile-test-url");
    const modal = document.getElementById("profile-test-modal");
    const modalOpen = !!(modal && modal.classList.contains("show"));
    const previousSelection = select ? select.value || "__default__" : "__default__";

    if (trigger) {
      trigger.classList.toggle("d-none", !hasProfiles);
      trigger.disabled = !hasProfiles;
    }
    if (separator) separator.classList.toggle("d-none", !hasProfiles);
    if (wrap) wrap.classList.toggle("d-none", !hasProfiles);

    if (!hasProfiles) {
      replaceSelectOptions(select, []);
      if (title) title.textContent = "Profile response";
      if (body) body.textContent = "Select a concrete group to test profile responses.";
      if (status) status.textContent = "—";
      if (contentType) contentType.textContent = "—";
      if (source) source.textContent = "—";
      if (url) {
        url.textContent = "—";
        url.removeAttribute("href");
      }
      return;
    }

    if (defaultProfile) {
      defaultProfile.textContent = data.scope_default_profile || "built-in-json";
    }
    if (defaultLink && data.scope_default_profile_href) {
      defaultLink.textContent = data.scope_default_profile_href;
      defaultLink.setAttribute("href", data.scope_default_profile_href);
    }

    replaceSelectOptions(select, data.profile_options, previousSelection);
    const selectedProfile = select && select.value ? select.value : "__default__";
    const href = resolveProfileHref(data.active_group, selectedProfile);

    if (title) {
      title.textContent = data.active_group
        ? `Profile response: ${data.active_group}`
        : "Profile response";
    }
    if (!modalOpen) {
      if (body) body.textContent = "Select a profile to fetch the live response.";
      if (status) status.textContent = "—";
      if (contentType) contentType.textContent = "—";
      if (source) source.textContent = href || "—";
      if (url) {
        url.textContent = href || "—";
        if (href) {
          url.setAttribute("href", href);
        } else {
          url.removeAttribute("href");
        }
      }
    }
  }

  // Keep popover placement consistent with the server-rendered buttons.
  // (User explicitly asked to keep it as-is.)
  let POPOVER_PLACEMENT = "left";

  const anyPopover = document.querySelector('[data-bs-toggle="popover"]');
  if (anyPopover) {
    const p = anyPopover.getAttribute("data-bs-placement");
    if (p) POPOVER_PLACEMENT = p;
  }

  function statusBadgeClass(status) {
    if (status === "up") return "badge bg-success-lt hc-status-badge";
    if (status === "warn") return "badge bg-warning-lt hc-status-badge";
    return "badge bg-danger-lt hc-status-badge";
  }

  function statusText(status) {
    if (status === "up") return "UP";
    if (status === "warn") return "WARN";
    return "DOWN";
  }

  function createButton({ kind, text, icon, className, popoverTitle, popoverHtml, checkName }) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = className;

    if (kind === "detail") {
      btn.classList.add("btn-check-detail");
      btn.setAttribute("data-check-name", checkName);
      btn.setAttribute("data-bs-toggle", "modal");
      btn.setAttribute("data-bs-target", "#check-detail-modal");
    } else {
      btn.setAttribute("data-action", kind);
      btn.setAttribute("data-bs-toggle", "popover");
      // Bootstrap recommends combining hover + focus for accessibility.
      // User requirement: must work on hover; focus is fine as an extra.
      btn.setAttribute("data-bs-trigger", "hover focus");
      btn.setAttribute("data-bs-placement", POPOVER_PLACEMENT);
      btn.setAttribute("data-bs-container", "body");
      btn.setAttribute("data-bs-custom-class", "hc-popover");
      btn.setAttribute("data-bs-title", popoverTitle);
      btn.setAttribute("data-bs-html", "true");
      btn.setAttribute("data-bs-sanitize", "false");
      btn.setAttribute("data-bs-content", popoverHtml);
    }

    const i = document.createElement("i");
    i.className = icon + " me-1";
    btn.appendChild(i);
    btn.appendChild(document.createTextNode(text));
    return btn;
  }

  function fillLastCell(tdLast, lastRunIso) {
    tdLast.innerHTML = "";

    if (lastRunIso) {
      const isoSpan = document.createElement("span");
      isoSpan.className = "font-monospace js-last-local";
      isoSpan.setAttribute("data-datetime", lastRunIso);
      isoSpan.setAttribute("title", lastRunIso);
      isoSpan.textContent = HealthzUi.formatLocalDateTime(lastRunIso) || lastRunIso;

      const agoSpan = document.createElement("span");
      agoSpan.className = "text-muted ms-1 hc-last-ago js-last-ago";
      agoSpan.setAttribute("data-datetime", lastRunIso);
      agoSpan.textContent = "(—)";

      tdLast.appendChild(isoSpan);
      tdLast.appendChild(agoSpan);
    } else {
      const muted = document.createElement("span");
      muted.className = "text-muted";
      muted.textContent = "—";
      tdLast.appendChild(muted);
    }
  }

  function createRow(check) {
    const tr = document.createElement("tr");
    tr.setAttribute("data-check-name", check.name);
    tr.setAttribute("data-check-status", check.status);

    // Check
    const tdName = document.createElement("td");
    tdName.className = "hc-col-check";
    const nameSpan = document.createElement("span");
    nameSpan.className = "hc-check-name";
    nameSpan.textContent = check.name;
    tdName.appendChild(nameSpan);

    // Last
    const tdLast = document.createElement("td");
    tdLast.className = "hc-col-last";
    fillLastCell(tdLast, check.last_run);

    // Status
    const tdStatus = document.createElement("td");
    tdStatus.className = "hc-col-status";
    const badge = document.createElement("span");
    badge.className = statusBadgeClass(check.status);
    badge.textContent = statusText(check.status);
    tdStatus.appendChild(badge);

    // Critical
    const tdCritical = document.createElement("td");
    tdCritical.className = "hc-col-critical";
    if (check.critical) {
      const cb = document.createElement("span");
      cb.className = "badge bg-primary-lt";
      cb.textContent = "Critical";
      tdCritical.appendChild(cb);
    }

    // Actions
    const tdActions = document.createElement("td");
    tdActions.className = "hc-col-actions";
    const actions = document.createElement("div");
    actions.className = "btn-list flex-nowrap justify-content-end hc-actions";
    tdActions.appendChild(actions);

    tr.appendChild(tdName);
    tr.appendChild(tdLast);
    tr.appendChild(tdStatus);
    tr.appendChild(tdCritical);
    tr.appendChild(tdActions);

    updateActions(tr, check);
    return tr;
  }

  function updateActions(tr, check) {
    const actions = tr.querySelector(".hc-actions");
    if (!actions) return;

    // dispose any existing popovers before changing buttons
    HealthzUi.disposePopovers(actions);
    actions.innerHTML = "";

    if (check.error && String(check.error).trim() !== "") {
      actions.appendChild(
        createButton({
          kind: "error",
          text: "Error",
          icon: "ti ti-alert-triangle",
          className: "btn btn-sm btn-outline-pink",
          popoverTitle: "Error",
          popoverHtml: HealthzUi.popoverHtmlForError(check.error),
          checkName: check.name,
        })
      );
    }

    if (Array.isArray(check.labels) && check.labels.length > 0) {
      actions.appendChild(
        createButton({
          kind: "labels",
          text: "Labels",
          icon: "ti ti-tags",
          className: "btn btn-sm btn-outline-primary",
          popoverTitle: "Labels",
          popoverHtml: HealthzUi.popoverHtmlForLabels(check.labels),
          checkName: check.name,
        })
      );
    }

    actions.appendChild(
      createButton({
        kind: "detail",
        text: "Detail",
        icon: "ti ti-eye",
        className: "btn btn-outline-secondary btn-sm",
        checkName: check.name,
      })
    );

    HealthzUi.initPopovers(actions);
  }

  function updateRow(tr, check) {
    // Keep the attribute in sync (used by filtering & lookup).
    tr.setAttribute("data-check-name", check.name);
    tr.setAttribute("data-check-status", check.status);

    // Name
    const nameSpan = tr.querySelector(".hc-check-name");
    if (nameSpan) nameSpan.textContent = check.name;

    // Last
    const tdLast = tr.querySelector("td.hc-col-last");
    if (tdLast) {
      const isoSpan = tdLast.querySelector(".js-last-local");
      const agoSpan = tdLast.querySelector(".js-last-ago");

      if (check.last_run) {
        if (isoSpan && agoSpan) {
          isoSpan.setAttribute("data-datetime", check.last_run);
          isoSpan.setAttribute("title", check.last_run);
          isoSpan.textContent =
            HealthzUi.formatLocalDateTime(check.last_run) || check.last_run;
          agoSpan.setAttribute("data-datetime", check.last_run);
        } else {
          fillLastCell(tdLast, check.last_run);
        }
      } else {
        fillLastCell(tdLast, null);
      }
    }

    // Status
    const badge = tr.querySelector(".hc-status-badge");
    if (badge) {
      badge.className = statusBadgeClass(check.status);
      badge.textContent = statusText(check.status);
    }

    // Critical
    const tdCritical = tr.querySelector("td.hc-col-critical");
    if (tdCritical) {
      tdCritical.innerHTML = "";
      if (check.critical) {
        const cb = document.createElement("span");
        cb.className = "badge bg-primary-lt";
        cb.textContent = "Critical";
        tdCritical.appendChild(cb);
      }
    }

    // Actions
    updateActions(tr, check);
  }

  function updateHeader(data) {
    const agg = document.getElementById("badge-aggregate");
    const aggValue = document.getElementById("badge-aggregate-value");
    if (agg && aggValue) {
      agg.classList.remove("bg-success-lt", "bg-danger-lt");
      if (data.aggregate_ok) {
        agg.classList.add("bg-success-lt");
        aggValue.textContent = "OK";
      } else {
        agg.classList.add("bg-danger-lt");
        aggValue.textContent = "FAILED";
      }
    }

    const uptime = document.getElementById("badge-uptime");
    if (uptime && data.uptime) uptime.textContent = data.uptime;

    const refresh = document.getElementById("badge-refresh");
    if (refresh && data.refresh_interval) refresh.textContent = data.refresh_interval;

    const scope = document.getElementById("badge-scope");
    if (scope && data.active_scope_label) scope.textContent = data.active_scope_label;

    const scopeHelp = document.getElementById("scope-help");
    if (scopeHelp && data.scope_help) scopeHelp.textContent = data.scope_help;

    const detailsLink = document.getElementById("details-link");
    if (detailsLink && data.details_href) detailsLink.setAttribute("href", data.details_href);

    const scopeHealthLink = document.getElementById("scope-health-link");
    if (scopeHealthLink && data.scope_health_href) {
      scopeHealthLink.setAttribute("href", data.scope_health_href);
      scopeHealthLink.textContent = data.scope_health_href;
    }

    const scopeDetailsLink = document.getElementById("scope-details-link");
    if (scopeDetailsLink && data.details_href) {
      scopeDetailsLink.setAttribute("href", data.details_href);
      scopeDetailsLink.textContent = data.details_href;
    }

    updateProfileUi(data);

    const groupSelect = document.getElementById("group-select");
    if (groupSelect && typeof data.active_group === "string") {
      groupSelect.value = data.active_group;
    }

    const updated = document.getElementById("badge-updated");
    if (updated && data.now) {
      updated.setAttribute("datetime", data.now);
      updated.textContent = data.now;
      HealthzUi.formatLocalTimes(document);
    }
  }

  function updateSummary(data) {
    const sum = document.getElementById("checks-summary");
    if (!sum) return;

    sum.textContent =
      `total=${data.summary_total}, up=${data.summary_up}, warn=${data.summary_warn}, down=${data.summary_down}, critical_down=${data.summary_critical_down}`;
  }

  function updateTable(data) {
    const tbody = document.getElementById("checks-tbody");
    if (!tbody) return;

    const existing = new Map();
    tbody.querySelectorAll("tr[data-check-name]").forEach((tr) => {
      const name = tr.getAttribute("data-check-name");
      if (name) existing.set(name, tr);
    });

    const checks = Array.isArray(data.checks) ? [...data.checks] : [];
    checks.sort((a, b) => String(a.name).localeCompare(String(b.name)));

    const desired = [];
    checks.forEach((c) => {
      const name = String(c.name || "");
      if (!name) return;
      const check = {
        name,
        status: String(c.status || "down"),
        critical: !!c.critical,
        last_run: c.last_run || null,
        error: c.error || "",
        labels: Array.isArray(c.labels) ? c.labels : [],
      };

      const tr = existing.get(name);
      if (tr) {
        updateRow(tr, check);
        desired.push(tr);
        existing.delete(name);
      } else {
        const newTr = createRow(check);
        desired.push(newTr);
      }
    });

    // Remove any rows that disappeared.
    existing.forEach((tr) => {
      HealthzUi.disposePopovers(tr);
      tr.remove();
    });

    // Re-attach in sorted order (moves existing nodes, appends new ones).
    desired.forEach((tr) => tbody.appendChild(tr));

    HealthzUi.formatLastLocal(tbody);

    // Update "(… ago)" parts.
    if (lastServerNowMs) {
      HealthzUi.formatLastAgo(tbody, lastServerNowMs);
    } else {
      HealthzUi.formatLastAgo(tbody);
    }

    // Re-apply filter after any DOM patch.
    applyFilter();
  }

  function applyFilter() {
    const tbody = document.getElementById("checks-tbody");
    if (!tbody) return;

    const q = String(filterQuery || "").trim().toLowerCase();
    const showUp = !!filterShowUp;
    const showWarn = !!filterShowWarn;
    const showDown = !!filterShowDown;

    const rows = tbody.querySelectorAll("tr[data-check-name]");
    rows.forEach((tr) => {
      const name = (tr.getAttribute("data-check-name") || "").toLowerCase();
      const status = (tr.getAttribute("data-check-status") || "").toLowerCase();

      const matchName = !q || name.includes(q);
      const matchStatus =
        (status === "up" && showUp) ||
        (status === "warn" && showWarn) ||
        (status === "down" && showDown);

      const visible = matchName && matchStatus;
      tr.classList.toggle("d-none", !visible);
    });
  }

  async function fetchSnapshot(href) {
    const resp = await fetch(href, {
      headers: { Accept: "application/json" },
      cache: "no-store",
    });
    if (!resp.ok) throw new Error(`HTTP ${resp.status} fetching ${href}`);
    return await resp.json();
  }

  async function refreshOnce() {
    if (inFlight) {
      refreshQueued = true;
      return;
    }
    inFlight = true;
    const href = currentSnapshotHref();
    try {
      const data = await fetchSnapshot(href);
      if (data && data.now) {
        const d = new Date(data.now);
        if (!isNaN(d.getTime())) lastServerNowMs = d.getTime();
      }
      if (data && typeof data.active_group === "string") {
        setActiveGroup(data.active_group);
      }
      updateHeader(data);
      updateSummary(data);
      updateTable(data);
    } catch (e) {
      // Keep UI stable; log only.
      console.warn("UI refresh failed:", e);
    } finally {
      inFlight = false;
      if (refreshQueued) {
        refreshQueued = false;
        refreshOnce();
      }
    }
  }

  document.addEventListener("DOMContentLoaded", () => {
    HealthzUi.formatLocalTimes(document);
    HealthzUi.formatLastLocal(document);
    HealthzUi.initPopovers(document);
    HealthzUi.formatLastAgo(document);

    // Keep "(… ago)" counters alive locally (lightweight).
    let tickLocal = Date.now();
    setInterval(() => {
      const nowLocal = Date.now();
      if (lastServerNowMs != null) {
        lastServerNowMs += nowLocal - tickLocal;
        HealthzUi.formatLastAgo(document, lastServerNowMs);
      } else {
        HealthzUi.formatLastAgo(document);
      }
      tickLocal = nowLocal;
    }, 1000);

    const root = document.getElementById("root");
    const secs = root && root.dataset && root.dataset.refreshSecs
      ? parseInt(root.dataset.refreshSecs, 10)
      : 30;
    const refreshMs = Math.max(1000, (isNaN(secs) ? 30 : secs) * 1000);
    setActiveGroup(getActiveGroup());

    // Initial refresh, then interval.
    refreshOnce();
    setInterval(refreshOnce, refreshMs);

    // Wire up filter input + status checkboxes + clear button.
    const filter = document.getElementById("check-filter");
    const clearBtn = document.getElementById("check-filter-clear");
    const groupSelect = document.getElementById("group-select");

    const cbUp = document.getElementById("filter-status-up");
    const cbWarn = document.getElementById("filter-status-warn");
    const cbDown = document.getElementById("filter-status-down");

    function readStatusCheckboxes() {
      filterShowUp = cbUp ? cbUp.checked : true;
      filterShowWarn = cbWarn ? cbWarn.checked : true;
      filterShowDown = cbDown ? cbDown.checked : true;
    }

    function isDefaultState() {
      const qEmpty = !filter || !(filter.value && String(filter.value).length);
      const allChecked =
        (!cbUp || cbUp.checked) &&
        (!cbWarn || cbWarn.checked) &&
        (!cbDown || cbDown.checked);
      return qEmpty && allChecked;
    }

    function syncFilterUi() {
      if (clearBtn) clearBtn.disabled = isDefaultState();
    }

    function applyAllFilters() {
      readStatusCheckboxes();
      applyFilter();
      syncFilterUi();
    }

    if (filter) {
      filter.addEventListener("input", () => {
        filterQuery = filter.value || "";
        applyAllFilters();
      });

      // Apply once on load (covers browser restoring input value).
      filterQuery = filter.value || "";
    }

    if (cbUp) cbUp.addEventListener("change", applyAllFilters);
    if (cbWarn) cbWarn.addEventListener("change", applyAllFilters);
    if (cbDown) cbDown.addEventListener("change", applyAllFilters);

    // Initial status state + filter application.
    applyAllFilters();

    if (clearBtn) {
      clearBtn.addEventListener("click", () => {
        if (filter) {
          filter.value = "";
          filterQuery = "";
        } else {
          filterQuery = "";
        }

        if (cbUp) cbUp.checked = true;
        if (cbWarn) cbWarn.checked = true;
        if (cbDown) cbDown.checked = true;

        applyAllFilters();

        if (filter) filter.focus();
      });
      syncFilterUi();
    }

    if (groupSelect) {
      groupSelect.addEventListener("change", () => {
        setActiveGroup(groupSelect.value || "");
        window.history.replaceState({}, "", currentUiHref());
        refreshOnce();
      });
    }
  });
})();
