// RecovMe front end. Talks to the Rust core through Tauri commands/events.
// Opened outside Tauri (or with ?demo), it runs a demo with sample data.
(() => {
  "use strict";
  const T = window.__TAURI__;
  const DEMO = !T || /[?&]demo\b/.test(location.search) || !!window.__RECOVME_DEMO__;
  const $ = (id) => document.getElementById(id);

  const state = {
    groups: [],
    selected: null, // Drive
    categories: new Set(["photos", "videos", "documents"]),
    mode: "smart",
    output: null,
    platform: "macos",
    view: "welcome",
    scanning: false,
    startedAt: 0,
    sessionDir: null,
  };

  // ── helpers ──
  const fmtBytes = (b) => {
    const u = ["B", "KB", "MB", "GB", "TB"];
    let i = 0;
    b = Number(b || 0);
    while (b >= 1000 && i < u.length - 1) { b /= 1000; i++; }
    return i === 0 ? `${b} B` : `${b >= 100 ? b.toFixed(0) : b.toFixed(1)} ${u[i]}`;
  };
  const fmtNum = (n) => Number(n || 0).toLocaleString();
  const fmtDur = (s) => {
    s = Math.max(0, Math.round(s));
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
    return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(x).padStart(2, "0")}` : `${m}:${String(x).padStart(2, "0")}`;
  };
  const fmtEta = (s) => {
    if (s == null) return "—";
    if (s < 60) return "< 1 min";
    if (s < 3600) return `${Math.round(s / 60)} min`;
    return `${Math.floor(s / 3600)} h ${Math.round((s % 3600) / 60)} min`;
  };
  const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  const imgSrc = (p) => (DEMO ? p : T.core.convertFileSrc(p));

  let toastTimer;
  function toast(msg) {
    const t = $("toast");
    t.textContent = msg;
    t.classList.remove("hidden");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => t.classList.add("hidden"), 4500);
  }

  // One dialog at a time: the rest of the window is made inert, Tab stays inside,
  // Escape closes, and focus returns to where it was.
  let closeOpenModal = null;
  function openModal(el, onClose, focusEl) {
    if (closeOpenModal) closeOpenModal();
    const back = document.activeElement;
    const app = document.querySelector(".app");
    app.inert = true;
    el.classList.remove("hidden");
    const onKey = (e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        close();
      } else if (e.key === "Tab") {
        const items = [...el.querySelectorAll("button:not(.hidden):not([disabled])")];
        if (!items.length) return;
        const i = items.indexOf(document.activeElement);
        const next = e.shiftKey ? (i <= 0 ? items.length - 1 : i - 1) : (i === items.length - 1 ? 0 : i + 1);
        e.preventDefault();
        items[next].focus();
      }
    };
    function close(result) {
      if (closeOpenModal !== close) return;
      closeOpenModal = null;
      el.classList.add("hidden");
      app.inert = false;
      document.removeEventListener("keydown", onKey, true);
      if (back && back.focus) back.focus();
      onClose(result);
    }
    document.addEventListener("keydown", onKey, true);
    closeOpenModal = close;
    (focusEl || el.querySelector("button")).focus();
    return close;
  }

  function dialog(title, text, okLabel = "Continue", alertOnly = false, focusCancel = false) {
    return new Promise((resolve) => {
      $("modalCancel").classList.toggle("hidden", alertOnly);
      $("modalTitle").textContent = title;
      $("modalText").textContent = text;
      $("modalOk").textContent = okLabel;
      const close = openModal($("modal"), (v) => resolve(v === true), focusCancel ? $("modalCancel") : $("modalOk"));
      $("modalOk").onclick = () => close(true);
      $("modalCancel").onclick = () => close(false);
    });
  }

  async function call(cmd, args) {
    if (DEMO) return Demo.call(cmd, args);
    return T.core.invoke(cmd, args);
  }

  // ── icons (line drawings, one stroke weight) ──
  const ICON = {
    removable: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M7.5 2.75h7l4 4v13.5a1 1 0 0 1-1 1h-10a1 1 0 0 1-1-1V3.75a1 1 0 0 1 1-1Z"/><path d="M9.5 3v3.5M12 3v3.5M14.5 3.5v3"/></svg>',
    external: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><rect x="2.75" y="6.25" width="18.5" height="11.5" rx="2.5"/><path d="M6.5 12h6.5"/><circle cx="17.2" cy="12" r="1" fill="currentColor"/></svg>',
    internal: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><rect x="3.25" y="3.75" width="17.5" height="16.5" rx="2.5"/><circle cx="12" cy="12" r="4.2"/><circle cx="12" cy="12" r=".9" fill="currentColor"/></svg>',
    image: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M6 2.75h8.5l4 4v14.5H6Z"/><path d="M14.5 2.75v4h4"/><path d="M9 13.5h6.5M9 16.5h6.5"/></svg>',
    volume: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><rect x="3.5" y="7" width="17" height="10" rx="2"/><path d="M7 12h5"/></svg>',
    photos: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><rect x="3" y="5" width="18" height="14" rx="2.5"/><circle cx="15.5" cy="9.5" r="1.6"/><path d="m3.5 17 5.2-5.2 4.3 4.3 2.4-2.4 5.1 4.8"/></svg>',
    videos: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><rect x="2.75" y="5.75" width="13.5" height="12.5" rx="2.5"/><path d="m16.25 10.2 5-3v9.6l-5-3"/></svg>',
    audio: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M9 17.5V5.5l11-2v12"/><circle cx="6.5" cy="17.5" r="2.5"/><circle cx="17.5" cy="15.5" r="2.5"/></svg>',
    documents: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M6 2.75h8.5l4 4v14.5H6Z"/><path d="M14.5 2.75v4h4M9 11.5h6.5M9 14.5h6.5M9 17.5h4"/></svg>',
    archives: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><rect x="3.25" y="4" width="17.5" height="4.5" rx="1.2"/><path d="M5 8.5v10.25a1.25 1.25 0 0 0 1.25 1.25h11.5A1.25 1.25 0 0 0 19 18.75V8.5M10 12h4"/></svg>',
    other: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M6 2.75h8.5l4 4v14.5H6Z"/><path d="M14.5 2.75v4h4"/></svg>',
    names: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M2.75 6.5a1.25 1.25 0 0 1 1.25-1.25h5l2 2.25h9a1.25 1.25 0 0 1 1.25 1.25v9.75A1.25 1.25 0 0 1 20 19.75H4a1.25 1.25 0 0 1-1.25-1.25Z"/><path d="m8.5 13.5 2.4 2.4 4.6-4.9"/></svg>',
  };

  const KIND_LABEL = { removable: "Card or USB stick", external: "External drive", internal: "Internal drive", image: "Disk image" };

  // ── drives ──
  async function loadDrives() {
    const btn = $("btnRefresh");
    btn.classList.add("spin");
    try {
      state.groups = await call("list_drives");
    } catch (e) {
      $("drives").innerHTML = `<p class="side-empty">Couldn't list drives: ${esc(e)}</p>`;
      btn.classList.remove("spin");
      return;
    }
    btn.classList.remove("spin");
    if (state.selected && state.selected.kind !== "image") {
      const still = allDrives().find((d) => d.id === state.selected.id);
      state.selected = still || null;
      if (!still && state.view === "setup") show("welcome");
    }
    renderDrives();
    if (state.view === "setup") renderSetup();
    validate();
  }

  const allDrives = () => state.groups.flatMap((g) => [g.disk, ...g.volumes]);

  function driveRow(d, isVol) {
    const ico = isVol ? ICON.volume : ICON[d.kind] || ICON.external;
    const sel = state.selected && state.selected.id === d.id ? " sel" : "";
    const sub = isVol ? d.detail : `${fmtBytes(d.size)} · ${KIND_LABEL[d.kind] || "Drive"}`;
    const flag = !isVol && d.warning ? '<span class="flag">Low Chance</span>' : "";
    return `<button class="drv${isVol ? " vol" : ""}${sel}" data-id="${esc(d.id)}" aria-current="${sel ? "true" : "false"}">
      <span class="ico">${ico}</span>
      <span class="txt"><span class="name">${esc(d.label)}</span><span class="sub">${esc(sub)}</span></span>
      ${flag}
    </button>`;
  }

  function renderDrives() {
    const parts = [];
    if (state.selected && state.selected.kind === "image") {
      parts.push(`<div class="disk">${driveRow(state.selected, false)}</div>`);
    }
    for (const g of state.groups) {
      parts.push(`<div class="disk">${driveRow(g.disk, false)}${g.volumes.map((v) => driveRow(v, true)).join("")}</div>`);
    }
    $("drives").innerHTML = parts.length ? parts.join("") : '<p class="side-empty">No drives found. Plug one in and press refresh.</p>';
    $("drives").querySelectorAll(".drv").forEach((el) => el.addEventListener("click", () => selectDrive(el.dataset.id)));
  }

  function findDrive(id) {
    if (state.selected && state.selected.id === id) return state.selected;
    return allDrives().find((d) => d.id === id) || null;
  }

  async function selectDrive(id) {
    if (state.scanning) {
      toast("A scan is running. Stop it first to choose another drive.");
      return;
    }
    const d = findDrive(id);
    if (!d) return;
    if (d.warning && d.isSystem) {
      const ok = await dialog("Scan This Drive Anyway?", `${d.warning} A scan of this drive can take a long time.`, "Scan Anyway");
      if (!ok) return;
    }
    state.selected = d;
    renderDrives();
    renderSetup();
    show("setup");
    validate();
  }

  function parentOf(d) {
    return state.groups.find((g) => g.volumes.some((v) => v.id === d.id));
  }

  function renderSetup() {
    const d = state.selected;
    if (!d) return;
    const isVol = !d.isWholeDisk;
    const parent = isVol ? parentOf(d) : null;
    const kind = parent ? parent.disk.kind : d.kind;
    $("driveArt").innerHTML = ICON[kind] || ICON.external;
    $("driveName").textContent = d.label;
    const bits = [];
    if (parent) bits.push(`On ${parent.disk.label}`);
    bits.push(d.detail || fmtBytes(d.size));
    if (!isVol && d.kind !== "image" && state.groups.find((g) => g.disk.id === d.id)?.volumes.length) {
      bits.push("Whole drive — use this after a format or when partitions are missing");
    }
    $("driveMeta").textContent = bits.join(" · ");
    const warning = d.warning || (parent && parent.disk.warning);
    const chance = $("driveChance");
    if (warning) {
      chance.textContent = "Low Chance";
      chance.className = "chance low";
    } else if (["removable", "external", "image"].includes(kind)) {
      chance.textContent = "Good Chance";
      chance.className = "chance";
    } else {
      chance.textContent = "";
    }
    $("driveWarn").textContent = warning || "";
    $("driveWarn").classList.toggle("hidden", !warning);
  }

  // ── options ──
  function bindOptions() {
    $("modes").addEventListener("keydown", (e) => {
      const keys = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };
      if (!(e.key in keys)) return;
      e.preventDefault();
      const items = [...$("modes").querySelectorAll(".mode")];
      const i = items.findIndex((m) => m.dataset.mode === state.mode);
      const next = items[(i + keys[e.key] + items.length) % items.length];
      next.click();
      next.focus();
    });
    $("chips").querySelectorAll(".chip").forEach((c) =>
      c.addEventListener("click", () => {
        const cat = c.dataset.cat;
        if (cat === "everything") {
          const on = !state.categories.has("everything");
          state.categories.clear();
          if (on) state.categories.add("everything");
        } else {
          state.categories.delete("everything");
          state.categories.has(cat) ? state.categories.delete(cat) : state.categories.add(cat);
        }
        $("chips").querySelectorAll(".chip").forEach((x) => {
          const on = state.categories.has(x.dataset.cat);
          x.classList.toggle("on", on);
          x.setAttribute("aria-pressed", on ? "true" : "false");
        });
        validate();
      })
    );
    $("modes").querySelectorAll(".mode").forEach((m) =>
      m.addEventListener("click", () => {
        state.mode = m.dataset.mode;
        $("modes").querySelectorAll(".mode").forEach((x) => {
          x.classList.toggle("on", x === m);
          x.setAttribute("aria-checked", x === m ? "true" : "false");
        });
        validate();
      })
    );
  }

  function scanOptions() {
    const d = state.selected;
    return {
      device: d.device,
      deviceLabel: d.label,
      bsdName: d.bsdName || null,
      filesystem: d.filesystem || null,
      isWholeDisk: !!d.isWholeDisk,
      sourceMounts: d.mountPoints || [],
      mode: state.mode,
      wholeArea: $("optWhole").checked,
      categories: [...state.categories],
      outputDir: state.output,
      skipSmall: $("optSmall").checked,
      dedupe: $("optDedupe").checked,
      renamePhotos: $("optRename").checked,
      unmount: state.platform === "macos" && $("optUnmount").checked,
    };
  }

  let validateSeq = 0;
  async function validate() {
    if (state.view !== "setup") {
      if (state.view === "welcome") {
        $("barText").textContent = "Choose a drive on the left to begin.";
        $("btnStart").disabled = true;
      }
      return;
    }
    const seq = ++validateSeq;
    const d = state.selected;
    const err = $("saveError");
    err.classList.add("hidden");
    let ok = !!d && !!state.output && state.categories.size > 0;
    let text = "";
    if (!d) text = "Choose a drive on the left to begin.";
    else if (state.categories.size === 0) text = "Choose at least one kind of file to look for.";
    else if (!state.output) text = "Choose where to save the recovered files.";
    if (d && state.mode === "quick" && !d.quickSupported) {
      ok = false;
      text = "Quick Scan can't read deleted names here. Pick a volume under the drive, or use Smart Scan.";
    }
    if (ok) {
      try {
        await call("check_output", { opts: scanOptions() });
      } catch (e) {
        if (seq !== validateSeq) return;
        err.textContent = String(e);
        err.classList.remove("hidden");
        ok = false;
        text = "Choose a save folder on another drive.";
      }
    }
    if (seq !== validateSeq) return;
    if (ok) {
      const what = state.categories.has("everything")
        ? "all file types"
        : [...state.categories].map((c) => ({ photos: "photos", videos: "videos", audio: "music", documents: "documents", archives: "archives" }[c])).join(", ");
      const modeName = { smart: "Smart Scan", quick: "Quick Scan", deep: "Deep Scan" }[state.mode];
      text = `${modeName} for ${what}.`;
    }
    $("barText").textContent = text;
    $("btnStart").disabled = !ok;
  }

  // ── views ──
  function show(view) {
    state.view = view;
    const map = { welcome: "viewWelcome", setup: "viewSetup", scan: "viewScan", done: "viewDone" };
    for (const [k, id] of Object.entries(map)) $(id).classList.toggle("hidden", k !== view);
    $("btnStart").classList.toggle("hidden", !(view === "setup" || view === "welcome"));
    $("btnStop").classList.toggle("hidden", view !== "scan");
    $("btnOpen").classList.toggle("hidden", view !== "done");
    $("btnAgain").classList.toggle("hidden", view !== "done");
    if (view === "scan") $("barText").textContent = "Keep the drive connected. Stopping keeps everything found so far.";
  }

  let tick;
  async function startScan() {
    const opts = scanOptions();
    $("btnStart").disabled = true;
    $("liveGrid").innerHTML = '<p class="empty">Photos appear here as soon as they are found.</p>';
    $("scanTitle").textContent = `Scanning ${state.selected.label}`;
    $("scanLabel").textContent = "Getting ready…";
    $("scanDetail").textContent = state.platform === "macos" ? "Your Mac will ask for your password so RecovMe can read the drive." : "";
    $("statFiles").textContent = "0";
    $("statPercent").textContent = "—";
    $("statEta").textContent = "—";
    $("statElapsed").textContent = "0:00";
    $("progressFill").style.width = "0%";
    $("progress").classList.add("indet");
    renderPhases(null);
    show("scan");
    $("btnStop").disabled = false;
    $("btnStop").textContent = "Stop";
    try {
      state.sessionDir = await call("start_scan", { opts });
    } catch (e) {
      show("setup");
      validate();
      dialog("Couldn't Start the Scan", String(e), "OK", true);
      return;
    }
    state.scanning = true;
    state.startedAt = Date.now();
    clearInterval(tick);
    tick = setInterval(() => ($("statElapsed").textContent = fmtDur((Date.now() - state.startedAt) / 1000)), 1000);
  }

  function renderPhases(current) {
    const list = [];
    if (state.mode !== "deep") list.push(["quick", "Quick Scan"]);
    if (state.mode !== "quick") list.push(["deep", "Deep Scan"]);
    list.push(["organize", "Organize"]);
    const order = list.map((x) => x[0]);
    const curIdx = order.indexOf(current);
    $("phases").innerHTML = list
      .map(([k, label], i) => `<li class="${k === current ? "now" : curIdx > i ? "ok" : ""}">${label}</li>`)
      .join("");
  }

  function onEvent(ev) {
    if (!state.scanning) return;
    if (ev.type === "progress") {
      $("scanLabel").textContent = ev.label;
      $("statFiles").textContent = fmtNum(ev.filesFound);
      $("scanDetail").textContent = ev.detail || "";
      const p = ev.percent;
      if (p == null) {
        $("progress").classList.add("indet");
        $("statPercent").textContent = ev.phase === "quick" ? "Quick" : "—";
      } else {
        $("progress").classList.remove("indet");
        $("progressFill").style.width = `${p.toFixed(1)}%`;
        $("statPercent").textContent = `${Math.floor(p)}%`;
      }
      $("statEta").textContent = fmtEta(ev.etaSecs);
      if (ev.phase !== "starting") renderPhases(ev.phase);
      if (ev.phase === "organize") {
        // Stopping now would change nothing; the files are being saved.
        $("btnStop").disabled = true;
        $("btnStop").textContent = "Saving…";
        if (closeOpenModal) closeOpenModal(false);
      }
    } else if (ev.type === "files") {
      addThumbs($("liveGrid"), ev.files.map((f) => f.path), 30, true);
    } else if (ev.type === "done") {
      finish(ev);
    } else if (ev.type === "error") {
      state.scanning = false;
      clearInterval(tick);
      show("setup");
      validate();
      dialog("The Scan Stopped", ev.message, "OK", true);
    }
  }

  function addThumbs(grid, paths, max, prepend) {
    if (!paths.length) return;
    for (const p of paths) {
      const div = document.createElement("div");
      div.className = "thumb";
      const img = document.createElement("img");
      img.loading = "lazy";
      img.alt = "";
      img.decoding = "async";
      img.src = imgSrc(p);
      img.onerror = () => div.remove();
      img.onload = () => grid.querySelector(".empty")?.remove();
      div.appendChild(img);
      prepend ? grid.prepend(div) : grid.appendChild(div);
    }
    const thumbs = grid.querySelectorAll(".thumb");
    for (let i = max; i < thumbs.length; i++) thumbs[i].remove();
  }

  function finish(s) {
    if (closeOpenModal) closeOpenModal(false);
    state.scanning = false;
    clearInterval(tick);
    state.sessionDir = s.sessionDir;
    $("doneTotal").textContent = fmtNum(s.total);
    $("doneWord").textContent = s.total === 1 ? "File" : "Files";
    $("doneEyebrow").textContent = s.stopped ? "Stopped early. Here is everything found until then." : `Finished in ${fmtDur(s.elapsedSecs)}`;
    const bits = [`${fmtBytes(s.totalBytes)} restored from ${state.selected ? state.selected.label : "the drive"}`];
    const count = (n, one, many) => `${fmtNum(n)} ${n === 1 ? one : many}`;
    if (s.duplicatesRemoved) bits.push(`${count(s.duplicatesRemoved, "duplicate", "duplicates")} removed`);
    if (s.smallSetAside) bits.push(`${count(s.smallSetAside, "tiny thumbnail", "tiny thumbnails")} set aside`);
    if (s.erased) bits.push(`${count(s.erased, "empty file", "empty files")} set aside`);
    if (s.stillOnDrive) bits.push(`${count(s.stillOnDrive, "file", "files")} still on the drive left out`);
    $("doneSub").textContent = bits.join(" · ");
    $("doneNotes").innerHTML = (s.notes || []).map((n) => `<div class="note info">${esc(n)}</div>`).join("");

    const rows = [];
    if (s.originalNames) {
      rows.push({ icon: "names", label: "With Original Names", sub: "Real names and folders", count: s.originalNames, path: s.originalNamesPath });
    }
    for (const c of s.categories || []) {
      rows.push({ icon: c.key, label: c.label, sub: fmtBytes(c.bytes), count: c.count, path: c.path });
    }
    $("results").innerHTML = rows.length
      ? rows
          .map(
            (r, i) => `<button class="row" role="listitem" data-i="${i}">
          <span class="ico">${ICON[r.icon] || ICON.other}</span>
          <span class="lbl">${esc(r.label)}<small>${esc(r.sub)}</small></span>
          <span class="cnt">${fmtNum(r.count)}</span>
          <span class="go">Open</span></button>`
          )
          .join("")
      : "";
    $("results").classList.toggle("hidden", !rows.length);
    $("results").querySelectorAll(".row").forEach((el) => el.addEventListener("click", () => openPath(rows[+el.dataset.i].path)));
    $("doneGrid").innerHTML = "";
    addThumbs($("doneGrid"), s.samplePhotos || [], 24, false);
    $("donePhotos").classList.toggle("hidden", !(s.samplePhotos || []).length);
    $("coffeeNote").classList.toggle("hidden", !(s.total > 0 && DONATE_URL));
    show("done");
    $("barText").textContent = s.total ? "Check the important files, then back them up." : "";
  }

  function showSavePath(p) {
    const el = $("savePath");
    if (!p) { el.textContent = "—"; return; }
    const trimmed = p.replace(/[\\/]+$/, "");
    const cut = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    const name = cut >= 0 ? trimmed.slice(cut + 1) : trimmed;
    const parent = cut > 0 ? trimmed.slice(0, cut) : "";
    el.innerHTML = `<b>${esc(name || p)}</b>${parent ? `<span>${esc(parent)}</span>` : ""}`;
    el.title = p;
    const cloud = /onedrive|icloud|mobile documents|dropbox|google ?drive|cloudstorage|box sync/i.test(p);
    $("saveCloud").classList.toggle("hidden", !cloud);
  }

  // Your PayPal.Me link, e.g. "https://paypal.me/YourName". While empty, the coffee
  // note and the "Support RecovMe" link stay hidden.
  const DONATE_URL = "https://paypal.me/dhandenvert";
  function openDonate() {
    const m = $("coffeeModal");
    const close = openModal(m, () => {}, $("coffeeOpen"));
    $("coffeeClose").onclick = () => close();
    m.onclick = (e) => {
      if (e.target === m) close();
    };
    $("coffeeOpen").onclick = async () => {
      try {
        await call("open_url", { url: DONATE_URL });
      } catch (e) {
        toast(`Couldn't open the browser: ${e}`);
      }
    };
  }

  async function openPath(p) {
    try {
      await call("open_path", { path: p });
    } catch (e) {
      toast(`Couldn't open the folder: ${e}`);
    }
  }

  // ── start ──
  async function init() {
    bindOptions();
    try {
      state.platform = await call("platform");
    } catch (_) {}
    document.body.classList.toggle("mac", state.platform === "macos");
    document.querySelectorAll(".mac-only").forEach((el) => el.classList.toggle("hidden", state.platform !== "macos"));
    try {
      const eng = await call("engine_status");
      const missing = [];
      if (!eng.photorec) missing.push("deep scan");
      if (!eng.tskRecover) missing.push("quick scan");
      $("engineBadge").innerHTML = missing.length
        ? `<span class="bad">Engine missing: ${missing.join(", ")}</span>`
        : "Free and open source software";
    } catch (_) {}
    try {
      state.output = await call("default_output_dir");
    } catch (_) {}
    showSavePath(state.output);
    if (!DEMO) {
      await T.event.listen("scan-event", (e) => onEvent(e.payload));
      await T.event.listen("app-closing", () => {
        $("btnStop").disabled = true;
        $("btnStop").textContent = "Stopping…";
        toast("Stopping the scan and saving what was found. RecovMe will close by itself.");
      });
    } else {
      Demo.listen(onEvent);
    }
    $("btnRefresh").onclick = loadDrives;
    $("btnImage").onclick = async () => {
      if (state.scanning) return toast("A scan is running. Stop it first.");
      const d = await call("pick_image").catch((e) => toast(String(e)));
      if (d) {
        state.selected = d;
        renderDrives();
        renderSetup();
        show("setup");
        validate();
      }
    };
    $("btnPick").onclick = async () => {
      const p = await call("pick_folder");
      if (p) {
        state.output = p;
        showSavePath(p);
        validate();
      }
    };
    $("btnStart").onclick = startScan;
    $("btnCoffee").onclick = openDonate;
    $("btnSupport").onclick = openDonate;
    $("btnSupport").classList.toggle("hidden", !DONATE_URL);
    $("btnStop").onclick = async () => {
      const ok = await dialog("Stop the Scan?", "Everything found so far is kept and organized.", "Stop Scan", false, true);
      if (!ok) return;
      $("btnStop").disabled = true;
      $("btnStop").textContent = "Stopping…";
      call("stop_scan");
    };
    $("btnOpen").onclick = () => state.sessionDir && openPath(state.sessionDir);
    $("btnAgain").onclick = () => {
      state.selected = null;
      show("welcome");
      validate();
      loadDrives();
    };
    for (const id of ["optWhole", "optUnmount"]) $(id).addEventListener("change", validate);
    show("welcome");
    validate();
    await loadDrives();
    if (DEMO) Demo.autoplay(selectDrive);
  }

  // ── demo mode ──
  const Demo = {
    handler: null,
    listen(h) { this.handler = h; },
    autoplay(select) {
      const m = /[?&]demo=(\w+)/.exec(location.search);
      const step = m ? m[1] : window.__RECOVME_DEMO__ || "";
      if (step === "setup" || step === "scan" || step === "done") {
        select("disk4s1").then(() => {
          if (step !== "setup") setTimeout(() => $("btnStart").click(), 150);
          if (step === "done") this.fast = true;
        });
      }
    },
    async call(cmd) {
      switch (cmd) {
        case "platform": return "macos";
        case "engine_status": return { photorec: "/x/photorec", tskRecover: "/x/tsk_recover" };
        case "default_output_dir": return "/Users/you/Desktop";
        case "check_output": return null;
        case "pick_folder": return "/Users/you/Desktop";
        case "pick_image": return null;
        case "open_path": return null;
        case "open_url": return null;
        case "stop_scan": return null;
        case "list_drives":
          return [
            { disk: { id: "disk4", label: "SanDisk Extreme SD", detail: "64.0 GB · Card", size: 64e9, device: "/dev/rdisk4", kind: "removable", isWholeDisk: true, quickSupported: false, mountPoints: ["/Volumes/CAMERA"] },
              volumes: [{ id: "disk4s1", label: "CAMERA", detail: "64.0 GB · ExFAT", size: 64e9, device: "/dev/rdisk4s1", kind: "removable", filesystem: "exfat", quickSupported: true, mountPoints: ["/Volumes/CAMERA"] }] },
            { disk: { id: "disk6", label: "WD My Passport", detail: "2.0 TB · External", size: 2e12, device: "/dev/rdisk6", kind: "external", isWholeDisk: true, quickSupported: false, mountPoints: ["/Volumes/Backup"] },
              volumes: [{ id: "disk6s2", label: "Backup", detail: "2.0 TB · NTFS", size: 2e12, device: "/dev/rdisk6s2", kind: "external", filesystem: "ntfs", quickSupported: true, mountPoints: ["/Volumes/Backup"] }] },
            { disk: { id: "disk0", label: "Apple SSD", detail: "1.0 TB · Internal", size: 1e12, device: "/dev/rdisk0", kind: "internal", isWholeDisk: true, isSystem: true, quickSupported: false, warning: "Mac internal drive: it is encrypted and uses TRIM, so deleted files can't be carved back. Check Trash, Time Machine or iCloud instead.", mountPoints: ["/"] },
              volumes: [] },
          ];
        case "start_scan":
          this.run();
          return "/Users/you/Desktop/Recovered - CAMERA";
      }
      return null;
    },
    run() {
      let p = 0, files = 0, phase = "quick";
      const scenes = [
        ["#9cc9e8", "#f6c453", "#3f7f6b", "#2b5d50"], ["#f4a77a", "#ffe08a", "#7a4f6d", "#4a3550"],
        ["#bfe3f2", "#fff3b0", "#5a9a6e", "#3d7a55"], ["#8fb8de", "#ffffff", "#6f8fa8", "#46637a"],
        ["#fbd3a5", "#ff9a5a", "#c76b4a", "#8e4a3a"], ["#a7d8de", "#fff7c2", "#e9d39a", "#2f8fa0"],
        ["#cdd9e6", "#f2f2f2", "#8a9aab", "#5e6e80"], ["#ffcfa8", "#ff7f50", "#3b5b7a", "#24405a"],
      ];
      const svg = (h) => {
        const [sky, sun, far, near] = scenes[Math.abs(Math.round(h)) % scenes.length];
        const x = 40 + (Math.round(h) * 13) % 40;
        return "data:image/svg+xml," + encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><rect width='100' height='100' fill='${sky}'/><circle cx='${x + 20}' cy='30' r='11' fill='${sun}'/><path d='M0 72 L22 48 L40 64 L62 38 L100 70 L100 100 L0 100Z' fill='${far}'/><path d='M0 86 L30 70 L55 82 L80 68 L100 80 L100 100 L0 100Z' fill='${near}'/></svg>`);
      };
      const fast = this.fast;
      const t = setInterval(() => {
        if (phase === "quick") {
          files += 19;
          this.handler({ type: "progress", phase, label: "Quick Scan — finding deleted files with their names", percent: null, filesFound: files, etaSecs: null, detail: "Reading the drive's file table" });
          if (files > 50) phase = "deep";
        } else if (phase === "deep") {
          p += fast ? 25 : 3; files += 23;
          this.handler({ type: "progress", phase, label: "Deep Scan — searching every block for lost files", percent: Math.min(p, 100), filesFound: files, etaSecs: Math.max(0, (100 - p) * 31), pass: 1, detail: `Pass 1 · ${(p * 0.64).toFixed(1)} GB of 64.0 GB read` });
          this.handler({ type: "files", files: [0, 1].map((i) => ({ path: svg(p / 3 * 2 + i), name: "x.jpg", category: "photos", size: 1 })) });
          if (p >= 100) phase = "organize";
        } else {
          clearInterval(t);
          this.handler({ type: "progress", phase: "organize", label: "Organizing your files", percent: 100, filesFound: files, etaSecs: null, detail: "Sorting by type, naming photos by date, removing duplicates" });
          setTimeout(() => this.handler({ type: "done", sessionDir: "/Users/you/Desktop/Recovered - CAMERA", originalNamesPath: "/x/Original names", total: 412, totalBytes: 2.9e9, originalNames: 58, duplicatesRemoved: 61, smallSetAside: 133, stopped: false, elapsedSecs: 1312, notes: [],
            categories: [{ key: "photos", label: "Photos", count: 287, bytes: 1.9e9, path: "/x/Photos" }, { key: "videos", label: "Videos", count: 21, bytes: 8.1e8, path: "/x/Videos" }, { key: "documents", label: "Documents", count: 46, bytes: 1.2e8, path: "/x/Documents" }],
            samplePhotos: Array.from({ length: 16 }, (_, i) => svg(i * 3 + 1)) }), 500);
        }
      }, fast ? 120 : 600);
    },
  };

  init();
})();
