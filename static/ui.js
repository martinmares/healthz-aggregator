// Theme toggle (Tabler / Bootstrap 5.3)
(function () {
  const key = "healthz-aggregator-ui-theme";
  const root = document.documentElement;
  const toggle = document.getElementById("theme-toggle");

  function setTheme(t) {
    root.setAttribute("data-bs-theme", t);
  }

  const saved = localStorage.getItem(key);
  const prefersDark =
    window.matchMedia &&
    window.matchMedia("(prefers-color-scheme: dark)").matches;
  const initial = saved || (prefersDark ? "dark" : "light");
  setTheme(initial);

  if (toggle) {
    toggle.checked = initial === "dark";
    toggle.addEventListener("change", () => {
      const next = toggle.checked ? "dark" : "light";
      setTheme(next);
      localStorage.setItem(key, next);
    });
  }
})();

// Convert RFC3339 timestamps (UTC) to browser-local time.
(function () {
  function formatLocalTimes(root) {
    const scope = root || document;
    const els = scope.querySelectorAll("time.js-localtime[datetime]");
    els.forEach((el) => {
      const dt = el.getAttribute("datetime");
      if (!dt) return;
      const d = new Date(dt);
      if (isNaN(d.getTime())) return;

      // Use browser locale settings.
      el.textContent = d.toLocaleString(undefined, {
        year: "numeric",
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      });
    });
  }

  document.addEventListener("DOMContentLoaded", () => formatLocalTimes(document));

  // Re-format after HTMX swaps.
  document.addEventListener("htmx:afterSettle", (evt) => {
    formatLocalTimes(evt.target || document);
  });
})();

// Bootstrap tooltips (Tabler ships Bootstrap bundle) + modal check details.
(function () {
  function initTooltips(root) {
    const scope = root || document;
    if (!window.bootstrap || !window.bootstrap.Tooltip) return;

    scope.querySelectorAll('[data-bs-toggle="tooltip"]').forEach((el) => {
      try {
        const existing = window.bootstrap.Tooltip.getInstance(el);
        if (existing) existing.dispose();
        new window.bootstrap.Tooltip(el);
      } catch (_) {
        // ignore
      }
    });
  }

  async function copyText(text) {
    if (!text) return;

    // Prefer modern Clipboard API.
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }

    // Fallback for older browsers.
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

  // Event delegation (works with HTMX swaps).
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
          // tiny visual feedback
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

    // Fetch and render.
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

  document.addEventListener("DOMContentLoaded", () => initTooltips(document));
  document.addEventListener("htmx:afterSettle", (evt) => initTooltips(evt.target || document));
})();
