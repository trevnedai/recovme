// RecovMe front end. Talks to the Rust core through Tauri commands/events.
// Opened outside Tauri (or with ?demo), it runs a demo with sample data.
(() => {
  "use strict";
  const T = window.__TAURI__;
  const DEMO = !T || /[?&]demo\b/.test(location.search) || !!window.__RECOVME_DEMO__;
  const $ = (id) => document.getElementById(id);

  const state = {
    groups: [],
    phones: [], // Android phones and iPhone / iPad backups
    selected: null, // Drive
    places: new Set(["trash", "thumbnails", "apps"]),
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
    android: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><rect x="6" y="2.75" width="12" height="18.5" rx="2.5"/><path d="M10.5 18.25h3"/></svg>',
    "ios-backup": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"><rect x="6" y="2.75" width="12" height="18.5" rx="3"/><path d="M12 7.5v6.5M9.5 11.5 12 14l2.5-2.5"/><path d="M10.5 18.25h3"/></svg>',
    phone: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><rect x="6" y="2.75" width="12" height="18.5" rx="2.5"/><path d="M10.5 18.25h3"/></svg>',
    trash: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"><path d="M4 6.5h16M9.5 6.5V4.25h5V6.5M6 6.5l1 13.25a1.25 1.25 0 0 0 1.25 1.25h7.5A1.25 1.25 0 0 0 17 19.75L18 6.5"/><path d="M12 10.5v6M9.5 13l2.5-2.5 2.5 2.5"/></svg>',
    chat: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M4 5.25h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-9l-4.5 3.5v-3.5H4a1 1 0 0 1-1-1v-10a1 1 0 0 1 1-1Z"/><path d="M7.5 10.5h9M7.5 13.5h5.5"/></svg>',
    names: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M2.75 6.5a1.25 1.25 0 0 1 1.25-1.25h5l2 2.25h9a1.25 1.25 0 0 1 1.25 1.25v9.75A1.25 1.25 0 0 1 20 19.75H4a1.25 1.25 0 0 1-1.25-1.25Z"/><path d="m8.5 13.5 2.4 2.4 4.6-4.9"/></svg>',
  };

  const KIND_LABEL = { removable: "Card or USB stick", external: "External drive", internal: "Internal drive", image: "Disk image", android: "Android phone", "ios-backup": "iPhone or iPad backup" };
  // What phone recovery can and can't do, said plainly. Shown before the first
  // phone scan and on the setup screen, so nobody starts with false hopes.
  const EXPECT = {
    android: {
      lead: "RecovMe can't search a phone block by block like a memory card. It can only copy files that still exist on the phone. Most fully deleted files are gone for good, and no app can bring them back.",
      can: [
        "Photos and videos deleted in about the last 30 days, while they are still in the phone's trash",
        "Files in the trash of Samsung, Xiaomi, Huawei, OPPO, vivo and other gallery and file apps",
        "Photos and videos you viewed in Telegram, and media WhatsApp and other chat apps still keep on the phone",
        "Photos and downloads that were never finished saving",
        "Small, low-quality previews (thumbnails) of some deleted photos, mostly on phones older than Android 10",
      ],
      cant: [
        "Files removed from the trash, or deleted more than about 30 days ago",
        "Anything after the phone was reset to factory settings",
        "Text messages, contacts and call history",
        "Photos that are only in the cloud (Google Photos, OneDrive…)",
      ],
      why: "<b>Why:</b> Android phones encrypt their storage, and the phone wipes the space of deleted files by itself. Once that happens, the data no longer exists anywhere on the phone.",
      first: "<b>Check these first:</b> the Trash in your Gallery or Google Photos app, and photos.google.com → Trash (kept 60 days). If the phone has a memory card, take it out and scan it in a card reader: that is where recovery works best.",
    },
    "ios-backup": {
      lead: "Apple doesn't let any app read an iPhone's storage directly, not even to look for deleted files. RecovMe can only read a backup that is already on this computer. Without one, nothing can be recovered.",
      can: [
        "Everything that was on the iPhone or iPad when this backup was made, even if you deleted it later",
        "Photos and videos, including those that were in Recently Deleted at that time",
        "Messages that were in Messages → Recently Deleted (iOS 16 and later), as a text file, with their photos",
        "Photos and videos sent in Messages and WhatsApp, the Hidden album, voice memos with their titles, Notes attachments and app documents",
      ],
      cant: [
        "Anything taken or saved after this backup was made",
        "Anything that was already deleted before this backup was made",
        "Backups that are only in iCloud (they live on Apple's servers)",
        "Full-size photos kept only in iCloud (“Optimize iPhone Storage”): at most a small preview",
        "Other deleted messages, and the text of WhatsApp and other app chats",
      ],
      why: "<b>Why:</b> a backup is a copy of the device at one moment. RecovMe reads that copy, so it can only give back what was in it.",
      first: "<b>Important:</b> don't make a new backup of this device yet: it replaces this one. To keep this one safe, open Manage Backups in Finder, iTunes or Apple Devices, right-click it and choose Archive. Check icloud.com → Photos → Recently Deleted too (kept 30 days).",
    },
  };
  const acknowledged = new Set();

  function fillList(el, items) {
    el.innerHTML = items.map((x) => `<li>${esc(x)}</li>`).join("");
  }

  // The first time a kind of phone is picked: say what to expect, and ask to confirm.
  function confirmExpectations(kind) {
    const e = EXPECT[kind];
    if (!e || acknowledged.has(kind)) return Promise.resolve(true);
    return new Promise((resolve) => {
      $("expectTitle").textContent = kind === "android" ? "Before You Start: Android Phones" : "Before You Start: iPhone and iPad";
      $("expectLead").textContent = e.lead;
      fillList($("expectModalCan"), e.can);
      fillList($("expectModalCant"), e.cant);
      $("expectModalFirst").innerHTML = e.first;
      const close = openModal($("expectModal"), (ok) => {
        if (ok === true) acknowledged.add(kind);
        resolve(ok === true);
      }, $("expectOk"));
      $("expectOk").onclick = () => close(true);
      $("expectCancel").onclick = () => close(false);
    });
  }

  const isPhone = (d) => !!d && (d.kind === "android" || d.kind === "ios-backup");

  // ── drives ──
  async function loadDrives() {
    const btn = $("btnRefresh");
    btn.classList.add("spin");
    // Phones answer more slowly than drives: list them on their own.
    const phones = call("list_phones").catch(() => []);
    try {
      state.groups = await call("list_drives");
    } catch (e) {
      $("drives").innerHTML = `<p class="side-empty">Couldn't list drives: ${esc(e)}</p>`;
      btn.classList.remove("spin");
      return;
    }
    refreshSelection();
    phones.then((p) => {
      state.phones = p || [];
      btn.classList.remove("spin");
      refreshSelection();
    });
  }

  function refreshSelection() {
    if (!state.scanning && state.selected && state.selected.kind !== "image" && !state.selected.picked) {
      const found = allDrives().find((d) => d.id === state.selected.id);
      const still = found && !found.blocked ? found : null;
      state.selected = still;
      if (!still && state.view === "setup") show("welcome");
    }
    renderDrives();
    if (state.view === "setup") renderSetup();
    validate();
  }

  const allDrives = () => [...state.groups.flatMap((g) => [g.disk, ...g.volumes]), ...state.phones];

  function driveRow(d, isVol) {
    const ico = isVol ? ICON.volume : ICON[d.kind] || ICON.external;
    const sel = state.selected && state.selected.id === d.id ? " sel" : "";
    const sub = isVol || isPhone(d) ? d.detail : `${fmtBytes(d.size)} · ${KIND_LABEL[d.kind] || "Drive"}`;
    const flag = d.blocked ? '<span class="flag">Action Needed</span>' : !isVol && d.warning && !isPhone(d) ? '<span class="flag">Low Chance</span>' : "";
    return `<button class="drv${isVol ? " vol" : ""}${sel}" data-id="${esc(d.id)}" aria-current="${sel ? "true" : "false"}">
      <span class="ico">${ico}</span>
      <span class="txt"><span class="name">${esc(d.label)}</span><span class="sub">${esc(sub)}</span></span>
      ${flag}
    </button>`;
  }

  function renderDrives() {
    const parts = [];
    const picked = state.selected && (state.selected.kind === "image" || state.selected.picked) ? state.selected : null;
    if (picked && picked.kind === "image") {
      parts.push(`<div class="disk">${driveRow(picked, false)}</div>`);
    }
    for (const g of state.groups) {
      parts.push(`<div class="disk">${driveRow(g.disk, false)}${g.volumes.map((v) => driveRow(v, true)).join("")}</div>`);
    }
    if (!parts.length) parts.push('<p class="side-empty">No drives found. Plug one in and press refresh.</p>');
    parts.push('<h3 class="side-sub">Phones &amp; Tablets</h3>');
    const phones = picked && isPhone(picked) && !state.phones.some((p) => p.id === picked.id) ? [picked, ...state.phones] : state.phones;
    for (const p of phones) parts.push(`<div class="disk">${driveRow(p, false)}</div>`);
    if (!phones.length) parts.push('<p class="side-empty">No phone or iPhone backup found.</p>');
    parts.push('<button class="side-help" id="btnPhoneHelp">Phone Not Listed?</button>');
    $("drives").innerHTML = parts.join("");
    $("drives").querySelectorAll(".drv").forEach((el) => el.addEventListener("click", () => selectDrive(el.dataset.id)));
    $("btnPhoneHelp").onclick = openPhoneHelp;
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
    if (d.blocked) {
      await dialog(d.kind === "android" ? "Allow This Computer on the Phone" : "One More Step", d.blocked, "OK", true);
      return;
    }
    if (isPhone(d) && !(await confirmExpectations(d.kind))) return;
    if (state.selected && state.selected.id !== d.id) $("backupPassword").value = "";
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
    const phone = isPhone(d);
    $("modesBlock").classList.toggle("hidden", phone);
    $("moreBlock").classList.toggle("hidden", phone);
    $("placesBlock").classList.toggle("hidden", d.kind !== "android");
    $("passwordBlock").classList.toggle("hidden", !(d.kind === "ios-backup" && d.encrypted));
    const e = EXPECT[d.kind];
    $("expectBlock").classList.toggle("hidden", !e);
    if (e) {
      fillList($("expectCan"), e.can);
      fillList($("expectCant"), e.cant);
      $("expectWhy").innerHTML = e.why;
      $("expectFirst").innerHTML = e.first;
    }
    $("driveInfo").textContent = d.kind === "android" ? "Nothing on the phone is changed: files are only copied off it." : d.kind === "ios-backup" ? "Nothing in the backup is changed: files are only copied out of it." : "";
    $("driveInfo").classList.toggle("hidden", !phone);
    $("btnStart").textContent = phone ? "Recover Files" : "Start Scan";
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
    if (phone) {
      chance.textContent = "";
    } else if (warning) {
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
    $("places").querySelectorAll(".chip").forEach((c) =>
      c.addEventListener("click", () => {
        const p = c.dataset.place;
        state.places.has(p) ? state.places.delete(p) : state.places.add(p);
        const on = state.places.has(p);
        c.classList.toggle("on", on);
        c.setAttribute("aria-pressed", on ? "true" : "false");
        validate();
      })
    );
    $("backupPassword").addEventListener("input", validate);
    $("backupPassword").addEventListener("keydown", (e) => {
      if (e.key === "Enter" && !$("btnStart").disabled) startScan();
    });
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

  function scanOptions(withPassword) {
    const d = state.selected;
    const encrypted = d.kind === "ios-backup" && d.encrypted;
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
      places: [...state.places],
      password: encrypted && withPassword ? $("backupPassword").value : null,
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
    if (ok && d.kind === "android" && state.places.size === 0) {
      ok = false;
      text = "Choose at least one place to look in.";
    }
    if (ok && d.kind === "ios-backup" && d.encrypted && !$("backupPassword").value) {
      ok = false;
      text = "Enter the backup's password.";
    }
    if (d && !isPhone(d) && state.mode === "quick" && !d.quickSupported) {
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
      text =
        d.kind === "android" ? `Copy deleted and forgotten ${what} from the phone.`
        : d.kind === "ios-backup" ? `Get ${what} out of the backup.`
        : `${modeName} for ${what}.`;
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
    if (view === "scan") {
      const d = state.selected;
      $("barText").textContent =
        d && d.kind === "android" ? "Keep the phone connected and unlocked. Stopping keeps everything copied so far."
        : d && d.kind === "ios-backup" ? "Stopping keeps everything copied so far."
        : "Keep the drive connected. Stopping keeps everything found so far.";
    }
  }

  let tick;
  async function startScan() {
    const opts = scanOptions(true);
    const phone = isPhone(state.selected);
    $("btnStart").disabled = true;
    $("liveGrid").innerHTML = '<p class="empty">Photos appear here as soon as they are found.</p>';
    $("scanTitle").textContent = phone ? `Recovering From ${state.selected.label}` : `Scanning ${state.selected.label}`;
    $("scanLabel").textContent = "Getting ready…";
    $("scanDetail").textContent = state.platform === "macos" && !phone ? "Your Mac will ask for your password so RecovMe can read the drive." : "";
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
    showUpdateBar();
    state.startedAt = Date.now();
    clearInterval(tick);
    tick = setInterval(() => ($("statElapsed").textContent = fmtDur((Date.now() - state.startedAt) / 1000)), 1000);
  }

  function renderPhases(current) {
    const list = [];
    const d = state.selected;
    if (isPhone(d)) {
      list.push(["copy", d.kind === "android" ? "Copy From Phone" : "Read Backup"]);
    } else {
      if (state.mode !== "deep") list.push(["quick", "Quick Scan"]);
      if (state.mode !== "quick") list.push(["deep", "Deep Scan"]);
    }
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
      showUpdateBar();
      clearInterval(tick);
      show("setup");
      validate();
      dialog(isPhone(state.selected) ? "Couldn't Finish" : "The Scan Stopped", ev.message, "OK", true);
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
    showUpdateBar();
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

  function openPhoneHelp() {
    const m = $("phoneModal");
    const close = openModal(m, () => {}, $("phoneClose"));
    $("phoneClose").onclick = () => close();
    m.onclick = (e) => {
      if (e.target === m) close();
    };
  }

  // ── updates ──
  // A newer version is found through GitHub's latest release, then downloaded,
  // checked against RecovMe's signing key, installed, and the app restarts.
  const AUTO_UPDATE_KEY = "recovme.autoUpdate";
  const autoUpdateOn = () => {
    try { return localStorage.getItem(AUTO_UPDATE_KEY) !== "off"; } catch (_) { return true; }
  };
  let pendingUpdate = null;
  let installing = false;

  function showUpdateBar() {
    const show = !!pendingUpdate && !state.scanning && !installing;
    $("updateBar").classList.toggle("hidden", !show && !installing);
    if (show) $("updateText").textContent = `RecovMe ${pendingUpdate.version} is available. You have ${pendingUpdate.current}.`;
  }

  async function checkUpdate(quiet) {
    try {
      pendingUpdate = await call("check_update");
    } catch (e) {
      if (!quiet) throw e;
      return;
    }
    showUpdateBar();
  }

  async function installUpdate() {
    if (state.scanning) return toast("A scan is running. Update when it has finished.");
    installing = true;
    $("updateBar").classList.remove("hidden");
    $("updateNow").disabled = true;
    $("updateLater").classList.add("hidden");
    $("updateText").textContent = "Downloading the update…";
    try {
      await call("install_update");
    } catch (e) {
      installing = false;
      $("updateNow").disabled = false;
      $("updateLater").classList.remove("hidden");
      showUpdateBar();
      dialog("Couldn't Update", String(e), "OK", true);
    }
  }

  function openUpdates() {
    const m = $("updateModal");
    $("optAutoUpdate").checked = autoUpdateOn();
    $("updateModalText").textContent = "Checking for a newer version…";
    $("updateModalInstall").classList.add("hidden");
    const close = openModal(m, () => {}, $("updateModalClose"));
    $("updateModalClose").onclick = () => close();
    $("updateModalInstall").onclick = () => {
      close();
      installUpdate();
    };
    $("optAutoUpdate").onchange = () => {
      try { localStorage.setItem(AUTO_UPDATE_KEY, $("optAutoUpdate").checked ? "on" : "off"); } catch (_) {}
    };
    checkUpdate(false)
      .then(() => {
        if (pendingUpdate) {
          $("updateModalText").textContent = `RecovMe ${pendingUpdate.version} is available (you have ${pendingUpdate.current}). Updating takes a minute, then RecovMe opens again.`;
          $("updateModalInstall").classList.remove("hidden");
        } else {
          $("updateModalText").textContent = "You have the latest version of RecovMe.";
        }
      })
      .catch(() => ($("updateModalText").textContent = "Couldn't reach GitHub to check for updates. Check the internet connection and try again."));
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
    document.querySelectorAll(".windows-only").forEach((el) => el.classList.toggle("hidden", state.platform !== "windows"));
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
    $("btnUpdates").onclick = openUpdates;
    $("updateNow").onclick = installUpdate;
    $("updateLater").onclick = () => {
      pendingUpdate = null;
      showUpdateBar();
    };
    try {
      const v = DEMO ? "0.3.0" : await T.app.getVersion();
      $("versionText").textContent = `Version ${v}`;
    } catch (_) {}
    if (!DEMO) {
      await T.event.listen("update-progress", (e) => {
        const [got, total] = e.payload;
        $("updateText").textContent = total
          ? `Downloading the update… ${Math.floor((got / total) * 100)}%`
          : `Downloading the update… ${fmtBytes(got)}`;
        if (total && got >= total) $("updateText").textContent = "Installing the update. RecovMe will open again by itself.";
      });
    }
    if (autoUpdateOn()) setTimeout(() => checkUpdate(true), 2500);
    $("btnImage").onclick = async () => {
      if (state.scanning) return toast("A scan is running. Stop it first.");
      const d = await call("pick_image").catch((e) => toast(String(e)));
      if (d) {
        if (d.blocked) return dialog("Can't Read This Backup", d.blocked, "OK", true);
        if (isPhone(d) && !(await confirmExpectations(d.kind))) return;
        if (isPhone(d)) d.picked = true;
        $("backupPassword").value = "";
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
      $("btnStart").textContent = "Start Scan";
      $("backupPassword").value = "";
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
        case "check_update": return /[?&]update\b/.test(location.search) ? { version: "0.3.1", current: "0.3.0" } : null;
        case "install_update": return new Promise(() => {});
        case "list_phones":
          return [
            { id: "adb:R58N", label: "Galaxy S23", detail: "Android 14 · USB", device: "adb:R58N", kind: "android", isWholeDisk: true, mountPoints: [] },
            { id: "iosbackup:/x", label: "Dhan's iPhone", detail: "iPhone backup · iOS 18.1 · 12 Mar 2026 · Encrypted", device: "iosbackup:/x", kind: "ios-backup", isWholeDisk: true, encrypted: true, mountPoints: ["/x"] },
          ];
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
