// Guide pages: point the download buttons straight at the newest installers.
// On any error they keep linking to the GitHub releases page.
(() => {
  const mac = document.querySelector("[data-dl=mac]");
  const win = document.querySelector("[data-dl=win]");
  if (!mac || !win) return;
  if (/Windows/i.test(navigator.userAgent || "")) {
    mac.classList.replace("btn-amber", "btn-ghost");
    win.classList.replace("btn-ghost", "btn-amber");
    win.parentNode.insertBefore(win, mac);
  }
  fetch("https://api.github.com/repos/trevnedai/recovme/releases/latest", { headers: { Accept: "application/vnd.github+json" } })
    .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
    .then((rel) => {
      const find = (re) => (rel.assets || []).find((a) => re.test(a.name));
      const dmg = find(/\.dmg$/i);
      const exe = find(/setup\.exe$/i) || find(/\.exe$/i);
      if (dmg) mac.href = dmg.browser_download_url;
      if (exe) win.href = exe.browser_download_url;
    })
    .catch(() => {});
})();
