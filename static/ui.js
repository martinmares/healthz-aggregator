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
