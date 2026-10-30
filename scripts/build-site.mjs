// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
// Builds the generated parts of recovme.app.
//
//   node scripts/build-site.mjs      (or: npm run site)
//
// Reads site-src/ and writes into site/:
//   guides/<slug>/, it/guide/<slug>/    guides and comparisons (site-src/guides/pages.json
//                                       + one body file per page and language)
//   guides/, it/guide/                  lists of guides and comparisons
//   about/, privacy/, terms/ and their Italian versions (site-src/pages/pages.json)
//   press/                              press kit (site-src/press.html)
//   it/index.html                       Italian home page, translated from site/index.html
//                                       with site-src/home.it.json
//   structured data on both home pages, sitemap.xml and llms.txt
// The generated files are committed, because GitHub Pages serves site/ as-is.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SITE = "https://recovme.app";
const src = (...p) => join(root, "site-src", ...p);
const out = (...p) => join(root, "site", ...p);
const { updated, guides } = JSON.parse(readFileSync(src("guides", "pages.json"), "utf8"));
const { pages } = JSON.parse(readFileSync(src("pages", "pages.json"), "utf8"));
const LANGS = ["en", "it"];

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const strip = (s) => s.replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim()
  .replace(/&amp;/g, "&").replace(/&nbsp;/g, " ").replace(/&rsquo;/g, "’");
const write = (file, html) => { mkdirSync(dirname(file), { recursive: true }); writeFileSync(file, html); };
const pathOut = (p) => out(...p.split("/").filter(Boolean), "index.html");

const L = {
  en: {
    root: "/", hub: "/guides/", hubName: "Guides", locale: "en_US", other: "it", otherName: "Italiano",
    home: "Home", questions: "Questions", download: "Download",
    by: "By Dhan Denvert Asuncion", updated: "Updated", platforms: "Mac and Windows",
    date: (d) => new Date(d + "T12:00:00Z").toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric" }),
    getTitle: "Get RecovMe. It’s free.",
    getText: "No trial, no size limit, no paywall after the scan. Nothing leaves your computer.",
    mac: "Download for Mac", win: "Download for Windows",
    req: "Mac with Apple silicon, macOS 11 or later · Windows 10 or 11, 64-bit",
    more: "More guides", moreCompare: "More comparisons", moreLink: "All guides",
    foot: "© 2026 Dhan Denvert Asuncion. Free and open source under GPL-3.0.",
    signing: "Code Signing Policy", source: "Source Code on GitHub", press: "Press",
    hubTitle: "File Recovery Guides | RecovMe",
    hubH1: "File Recovery Guides",
    hubLede: "Step-by-step help for the most common ways files get lost, with an honest answer about your chances first. Every guide uses RecovMe, which is free with no limits.",
    hubDesc: "Free step-by-step guides to recover deleted photos, videos and files from SD cards, USB sticks, hard drives, Android phones and iPhone backups on Mac and Windows.",
    hubGuides: "Recovery guides", hubCompare: "Free alternatives and comparisons",
    read: "Read the guide", readCompare: "Read the comparison",
  },
  it: {
    root: "/it/", hub: "/it/guide/", hubName: "Guide", locale: "it_IT", other: "en", otherName: "English",
    home: "Home", questions: "Domande", download: "Scarica",
    by: "Di Dhan Denvert Asuncion", updated: "Aggiornata il", platforms: "Mac e Windows",
    date: (d) => new Date(d + "T12:00:00Z").toLocaleDateString("it-IT", { day: "numeric", month: "long", year: "numeric" }),
    getTitle: "Scarica RecovMe. È gratis.",
    getText: "Nessuna prova, nessun limite di dimensione, niente da pagare dopo la scansione. I tuoi file non lasciano mai il computer.",
    mac: "Scarica per Mac", win: "Scarica per Windows",
    req: "Mac con Apple silicon, macOS 11 o successivo · Windows 10 o 11, 64 bit",
    more: "Altre guide", moreCompare: "Altri confronti", moreLink: "Tutte le guide",
    foot: "© 2026 Dhan Denvert Asuncion. Gratis e open source con licenza GPL-3.0.",
    signing: "Firma del codice", source: "Codice sorgente su GitHub", press: "Stampa",
    hubTitle: "Guide al recupero dati gratis | RecovMe",
    hubH1: "Guide al recupero dei file",
    hubLede: "Aiuto passo passo per i modi più comuni in cui si perdono i file, con prima una risposta onesta sulle probabilità di recupero. Ogni guida usa RecovMe, gratis e senza limiti.",
    hubDesc: "Guide gratuite passo passo per recuperare foto, video e file cancellati da schede SD, chiavette USB, hard disk, telefoni Android e backup di iPhone su Mac e Windows.",
    hubGuides: "Guide al recupero", hubCompare: "Alternative gratuite e confronti",
    read: "Leggi la guida", readCompare: "Leggi il confronto",
  },
};

// Simple pages (About, Privacy, Terms) by id, for links in bylines and footers.
const page = Object.fromEntries(pages.map((p) => [p.id, p]));
const url = (lang, slug) => `${SITE}${L[lang].hub}${slug ? slug + "/" : ""}`;
const path = (lang, slug) => `${L[lang].hub}${slug ? slug + "/" : ""}`;
const kindOf = (g) => g.kind || "guide";
const guideDate = (g) => g.updated || (g.published > updated ? g.published : updated);

const ICON_MAC = '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M16.4 12.6c0-2.3 1.9-3.4 2-3.5-1.1-1.6-2.8-1.8-3.4-1.8-1.4-.1-2.8.9-3.5.9-.7 0-1.8-.8-3-.8-1.5 0-3 .9-3.8 2.3-1.6 2.8-.4 7 1.2 9.3.8 1.1 1.7 2.4 2.9 2.3 1.2 0 1.6-.7 3-.7s1.8.7 3 .7c1.3 0 2.1-1.1 2.8-2.3.9-1.3 1.3-2.6 1.3-2.6s-2.5-1-2.5-3.8ZM14.1 5.9c.6-.8 1.1-1.8 1-2.9-.9 0-2.1.6-2.7 1.4-.6.7-1.1 1.8-1 2.8 1 .1 2.1-.5 2.7-1.3Z"/></svg>';
const ICON_WIN = '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M3 5.5 10 4.5v7H3zm8-1.1L21 3v8.5H11zM3 12.5h7v7L3 18.5zm8 0h10V21l-10-1.4z"/></svg>';
const RELEASES = "https://github.com/trevnedai/recovme/releases/latest";

// Who makes RecovMe, shared by every page's structured data.
const person = {
  "@type": "Person", "@id": SITE + "/#maker", name: "Dhan Denvert Asuncion",
  url: SITE + page.about.en.path, sameAs: ["https://github.com/trevnedai"],
};
const org = {
  "@type": "Organization", "@id": SITE + "/#org", name: "RecovMe", url: SITE + "/",
  logo: { "@type": "ImageObject", url: SITE + "/assets/icon-512.png", width: 512, height: 512 },
  sameAs: ["https://github.com/trevnedai/recovme"], founder: { "@id": person["@id"] },
};
const app = { "@type": "SoftwareApplication", "@id": SITE + "/#app", name: "RecovMe", url: SITE + "/" };

function head({ lang, title, desc, canonical, alternates, type, ld }) {
  const alt = alternates
    ? Object.entries(alternates).map(([l, u]) => `<link rel="alternate" hreflang="${l}" href="${u}">`).join("\n") +
      `\n<link rel="alternate" hreflang="x-default" href="${alternates.en}">`
    : "";
  return `<!doctype html>
<html lang="${lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>${esc(title)}</title>
<meta name="description" content="${esc(desc)}">
<meta name="author" content="Dhan Denvert Asuncion">
<link rel="canonical" href="${canonical}">
${alt}
<link rel="icon" href="/assets/favicon.png" sizes="64x64" type="image/png">
<link rel="icon" href="/assets/icon-192.png" sizes="192x192" type="image/png">
<link rel="apple-touch-icon" href="/assets/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">
<meta name="theme-color" content="#0e3b39">
<meta property="og:site_name" content="RecovMe">
<meta property="og:title" content="${esc(title)}">
<meta property="og:description" content="${esc(desc)}">
<meta property="og:image" content="${SITE}/assets/og.png">
<meta property="og:url" content="${canonical}">
<meta property="og:type" content="${type}">
<meta property="og:locale" content="${L[lang].locale}">
<meta name="twitter:card" content="summary_large_image">
<link rel="stylesheet" href="/assets/fonts/fonts.css">
<link rel="stylesheet" href="/assets/guide.css">
<script type="application/ld+json">
${JSON.stringify(ld, null, 1)}
</script>
</head>
<body>
`;
}

function topbar(lang, otherUrl, current) {
  const t = L[lang];
  return `<header class="top">
  <div class="wrap">
    <a class="brand" href="${t.root}"><img src="/assets/icon-256.png" width="36" height="36" alt="">RecovMe</a>
    <nav aria-label="Site">
      <a href="${t.hub}"${current ? ' aria-current="page"' : ""}>${t.hubName}</a>
      <a href="${t.root}#questions">${t.questions}</a>
      <a class="lang" href="${otherUrl}" hreflang="${t.other}" lang="${t.other}">${t.otherName}</a>
      <a class="get" href="#download">${t.download}</a>
    </nav>
  </div>
</header>
`;
}

function getBox(lang) {
  const t = L[lang];
  return `<section class="get-box" id="download" aria-labelledby="download-title">
  <img src="/assets/icon-256.png" width="76" height="76" alt="">
  <div><h2 id="download-title">${t.getTitle}</h2><p>${t.getText}</p></div>
  <div class="btns">
    <a class="btn btn-amber" data-dl="mac" href="${RELEASES}">${ICON_MAC}${t.mac}</a>
    <a class="btn btn-ghost" data-dl="win" href="${RELEASES}">${ICON_WIN}${t.win}</a>
  </div>
  <p class="req">${t.req}</p>
</section>
`;
}

function footer(lang) {
  const t = L[lang];
  const p = (id) => `<a href="${page[id][lang].path}">${page[id][lang].short}</a>`;
  return `<footer class="foot">
  <div class="wrap">
    <p>${t.foot}</p>
    <p><a href="${t.root}">${t.home}</a> · <a href="${t.hub}">${t.hubName}</a> · ${p("about")} · ${p("privacy")} · ${p("terms")} · <a href="/press/">${t.press}</a> · <a href="/code-signing.html">${t.signing}</a> · <a href="https://github.com/trevnedai/recovme" rel="noopener">${t.source}</a></p>
  </div>
</footer>
<script src="/assets/guide.js" defer></script>
</body>
</html>
`;
}

const card = (lang, g) => `<li><a href="${path(lang, g[lang].slug)}"><b>${esc(g[lang].h1)}</b><span>${esc(g[lang].card)}</span><span class="tag">${kindOf(g) === "compare" ? L[lang].readCompare : L[lang].read} →</span></a></li>`;

// Questions and answers in a page's FAQ, for FAQPage structured data.
function faqs(body) {
  const sec = body.split('class="faq"')[1] || "";
  return [...sec.matchAll(/<summary>([\s\S]*?)<\/summary>\s*<p>([\s\S]*?)<\/p>/g)]
    .map(([, q, a]) => ({ "@type": "Question", name: strip(q), acceptedAnswer: { "@type": "Answer", text: strip(a) } }));
}

const sitemap = []; // [url, lastmod, alternates?]

// Guides and comparisons
for (const g of guides) {
  for (const lang of LANGS) {
    const t = L[lang], p = g[lang], other = t.other;
    const body = readFileSync(src("guides", `${g.id}.${lang}.html`), "utf8").trim();
    const canonical = url(lang, p.slug);
    const date = guideDate(g);
    const q = faqs(body);
    const ld = {
      "@context": "https://schema.org",
      "@graph": [
        {
          "@type": "Article", headline: p.h1, description: p.description, inLanguage: lang,
          datePublished: g.published, dateModified: date, author: { "@id": person["@id"] }, publisher: { "@id": org["@id"] },
          image: SITE + "/assets/og.png", mainEntityOfPage: canonical, about: app,
        },
        person, org,
        {
          "@type": "BreadcrumbList", itemListElement: [
            { "@type": "ListItem", position: 1, name: "RecovMe", item: SITE + t.root },
            { "@type": "ListItem", position: 2, name: t.hubName, item: url(lang) },
            { "@type": "ListItem", position: 3, name: p.short, item: canonical },
          ],
        },
        ...(q.length ? [{ "@type": "FAQPage", mainEntity: q }] : []),
      ],
    };
    const same = guides.filter((x) => x.id !== g.id && kindOf(x) === kindOf(g));
    const others = guides.filter((x) => kindOf(x) !== kindOf(g)).slice(0, 2);
    const related = [...same, ...others].slice(0, 5).map((x) => card(lang, x)).join("\n");
    const alternates = { en: url("en", g.en.slug), it: url("it", g.it.slug) };
    const html = head({ lang, title: p.title, desc: p.description, canonical, type: "article", ld, alternates })
      + topbar(lang, path(other, g[other].slug), false) + `
<main>
  <div class="wrap">
    <ol class="crumbs"><li><a href="${t.root}">RecovMe</a></li><li><a href="${t.hub}">${t.hubName}</a></li><li aria-current="page">${esc(p.short)}</li></ol>
    <div class="head">
      <h1>${esc(p.h1)}</h1>
      <p class="lede">${p.lede}</p>
      <p class="meta"><a href="${page.about[lang].path}">${t.by}</a><span>${t.updated} ${t.date(date)}</span><span>${t.platforms}</span></p>
    </div>
    <article>
${body}
    </article>
${getBox(lang)}
    <section class="related" aria-labelledby="more-title">
      <h2 id="more-title">${kindOf(g) === "compare" ? t.moreCompare : t.more}</h2>
      <ul class="cards">
${related}
      </ul>
      <p><a href="${t.hub}">${t.moreLink} →</a></p>
    </section>
  </div>
</main>
` + footer(lang);
    write(out(...t.hub.split("/").filter(Boolean), p.slug, "index.html"), html);
  }
  const alts = { en: url("en", g.en.slug), it: url("it", g.it.slug) };
  sitemap.push([alts.en, guideDate(g), alts], [alts.it, guideDate(g), alts]);
}

// Lists of guides and comparisons
const newest = guides.map(guideDate).sort().at(-1);
for (const lang of LANGS) {
  const t = L[lang];
  const ld = {
    "@context": "https://schema.org",
    "@graph": [
      { "@type": "CollectionPage", name: t.hubH1, description: t.hubDesc, inLanguage: lang, url: url(lang), about: app },
      { "@type": "ItemList", itemListElement: guides.map((g, i) => ({ "@type": "ListItem", position: i + 1, url: url(lang, g[lang].slug), name: g[lang].h1 })) },
    ],
  };
  const list = (kind) => guides.filter((g) => kindOf(g) === kind).map((g) => card(lang, g)).join("\n");
  const html = head({ lang, title: t.hubTitle, desc: t.hubDesc, canonical: url(lang), type: "website", ld, alternates: { en: url("en"), it: url("it") } })
    + topbar(lang, L[t.other].hub, true) + `
<main>
  <div class="wrap">
    <ol class="crumbs"><li><a href="${t.root}">RecovMe</a></li><li aria-current="page">${t.hubName}</li></ol>
    <div class="head">
      <h1>${t.hubH1}</h1>
      <p class="lede">${t.hubLede}</p>
    </div>
    <h2 class="list-title">${t.hubGuides}</h2>
    <ul class="cards">
${list("guide")}
    </ul>
    <h2 class="list-title">${t.hubCompare}</h2>
    <ul class="cards">
${list("compare")}
    </ul>
${getBox(lang)}
  </div>
</main>
` + footer(lang);
  write(out(...t.hub.split("/").filter(Boolean), "index.html"), html);
}
sitemap.push([url("en"), newest, { en: url("en"), it: url("it") }], [url("it"), newest, { en: url("en"), it: url("it") }]);

// About, Privacy and Terms
for (const pg of pages) {
  for (const lang of LANGS) {
    const t = L[lang], p = pg[lang];
    const body = readFileSync(src("pages", `${pg.id}.${lang}.html`), "utf8").trim();
    const canonical = SITE + p.path;
    const alternates = { en: SITE + pg.en.path, it: SITE + pg.it.path };
    const ld = {
      "@context": "https://schema.org",
      "@graph": [
        { "@type": pg.type || "WebPage", name: p.h1, description: p.description, url: canonical, inLanguage: lang, dateModified: pg.updated,
          ...(pg.id === "about" ? { mainEntity: { "@id": person["@id"] } } : {}), about: app },
        person, org,
      ],
    };
    const html = head({ lang, title: p.title, desc: p.description, canonical, type: "website", ld, alternates })
      + topbar(lang, pg[t.other].path, false) + `
<main>
  <div class="wrap">
    <ol class="crumbs"><li><a href="${t.root}">RecovMe</a></li><li aria-current="page">${esc(p.short)}</li></ol>
    <div class="head">
      <h1>${esc(p.h1)}</h1>
      ${p.lede ? `<p class="lede">${p.lede}</p>` : ""}
      <p class="meta"><span>${t.updated} ${t.date(pg.updated)}</span></p>
    </div>
    <article>
${body}
    </article>
${getBox(lang)}
  </div>
</main>
` + footer(lang);
    write(pathOut(p.path), html);
  }
  sitemap.push([SITE + pg.en.path, pg.updated, { en: SITE + pg.en.path, it: SITE + pg.it.path }],
    [SITE + pg.it.path, pg.updated, { en: SITE + pg.en.path, it: SITE + pg.it.path }]);
}

// Press kit (English, with an Italian paragraph)
{
  const body = readFileSync(src("press.html"), "utf8").trim();
  const canonical = SITE + "/press/";
  const desc = "RecovMe press kit: a short description, key facts, the app icon, a banner and screenshots to use in articles, reviews and videos.";
  const ld = { "@context": "https://schema.org", "@graph": [{ "@type": "WebPage", name: "RecovMe Press Kit", description: desc, url: canonical, inLanguage: "en", about: app }, person, org] };
  const html = head({ lang: "en", title: "Press Kit | RecovMe", desc, canonical, type: "website", ld }) + topbar("en", "/it/", false) + `
<main>
  <div class="wrap">
    <ol class="crumbs"><li><a href="/">RecovMe</a></li><li aria-current="page">Press Kit</li></ol>
    <div class="head"><h1>Press Kit</h1></div>
    <article>
${body}
    </article>
${getBox("en")}
  </div>
</main>
` + footer("en");
  write(out("press", "index.html"), html);
  sitemap.push([canonical, updated]);
}

// Italian home page: built from the English one with the snippets in
// site-src/home.it.json. Snippets that no longer match are listed, so a change
// to the English page never slips through untranslated without a warning.
{
  const en = readFileSync(out("index.html"), "utf8");
  const { pairs } = JSON.parse(readFileSync(src("home.it.json"), "utf8"));
  let it = en;
  const missing = [];
  for (const [from, to] of [...pairs].sort((a, b) => b[0].length - a[0].length)) {
    if (!it.includes(from)) { missing.push(from); continue; }
    it = it.split(from).join(to);
  }
  it = it
    .replace(/(src|href)="assets\//g, '$1="/assets/')
    .replace(/url\(assets\//g, "url(/assets/")
    .replace(/href="code-signing\.html"/g, 'href="/code-signing.html"');
  if (missing.length) {
    console.warn(`\nItalian home page: ${missing.length} English snippet(s) not found. Update site-src/home.it.json:`);
    for (const m of missing) console.warn("  - " + m.slice(0, 110));
  }
  write(out("it", "index.html"), it);
}

// Home page structured data: the app, its maker and the Questions section,
// rebuilt from each page itself so it never drifts from what visitors see.
for (const [file, lang, pageUrl] of [[out("index.html"), "en", SITE + "/"], [out("it", "index.html"), "it", SITE + "/it/"]]) {
  const html = readFileSync(file, "utf8");
  const questions = (html.split('id="questions"')[1] || "").split("</section>")[0];
  const ld = {
    "@context": "https://schema.org",
    "@graph": [
      { "@type": "WebSite", "@id": SITE + "/#site", name: "RecovMe", url: SITE + "/", inLanguage: ["en", "it"], publisher: { "@id": org["@id"] } },
      org, person,
      {
        "@type": "SoftwareApplication", "@id": app["@id"], name: "RecovMe", url: SITE + "/",
        description: strip(html.match(/<meta name="description" content="([^"]+)"/)[1]),
        applicationCategory: "UtilitiesApplication", applicationSubCategory: "Data recovery",
        operatingSystem: "macOS 11 or later (Apple silicon), Windows 10, Windows 11",
        offers: { "@type": "Offer", price: "0", priceCurrency: "EUR" },
        isAccessibleForFree: true,
        license: "https://www.gnu.org/licenses/gpl-3.0.html",
        downloadUrl: RELEASES,
        releaseNotes: "https://github.com/trevnedai/recovme/blob/main/CHANGELOG.md",
        screenshot: ["app-setup.webp", "app-scan.webp", "app-done.webp"].map((f) => `${SITE}/assets/${f}`),
        image: SITE + "/assets/og.png",
        author: { "@id": person["@id"] }, publisher: { "@id": org["@id"] },
        codeRepository: "https://github.com/trevnedai/recovme",
      },
      { "@type": "FAQPage", "@id": pageUrl + "#questions", inLanguage: lang, mainEntity: faqs('class="faq"' + questions) },
    ],
  };
  const tag = /<script type="application\/ld\+json" id="ld">[\s\S]*?<\/script>/;
  if (!tag.test(html)) throw new Error(file + " is missing <script type=\"application/ld+json\" id=\"ld\">");
  writeFileSync(file, html.replace(tag, `<script type="application/ld+json" id="ld">\n${JSON.stringify(ld, null, 1)}\n</script>`));
}
sitemap.unshift([SITE + "/", updated, { en: SITE + "/", it: SITE + "/it/" }], [SITE + "/it/", updated, { en: SITE + "/", it: SITE + "/it/" }]);
sitemap.push([SITE + "/code-signing.html", updated]);

// Sitemap
write(out("sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${sitemap.map(([loc, lastmod, alts]) => `  <url>
    <loc>${loc}</loc>
    <lastmod>${lastmod}</lastmod>${alts ? "\n" + Object.entries({ ...alts, "x-default": alts.en }).map(([l, u]) => `    <xhtml:link rel="alternate" hreflang="${l}" href="${u}"/>`).join("\n") : ""}
  </url>`).join("\n")}
</urlset>
`);

// llms.txt (llmstxt.org): a plain summary and map of the site for AI tools.
{
  const lines = (kind, lang) => guides.filter((g) => kindOf(g) === kind)
    .map((g) => `- [${g[lang].h1}](${url(lang, g[lang].slug)}): ${strip(g[lang].card)}`).join("\n");
  write(out("llms.txt"), `# RecovMe

> RecovMe is a free, open-source (GPL-3.0) file and photo recovery app for Mac (Apple silicon, macOS 11 or later) and Windows 10/11. It recovers deleted and formatted photos, videos, music, recordings, CCTV and dashcam footage and documents from SD cards, USB sticks, external drives and PC hard drives, and copies what still survives on Android phones and in iPhone or iPad backups. It has no trial, no size limit and no paywall after the scan, needs no account, and nothing leaves the computer.

Made by Dhan Denvert Asuncion. It runs two open-source recovery engines in one scan: The Sleuth Kit (recently deleted files with their original names) and PhotoRec (480+ file types found by content, even after a format), then sorts the results by type, names photos and videos by the date they were taken and removes duplicates. It only reads the drive it scans and refuses to save onto it.

Honest limits: no app can recover files deleted from internal SSDs with TRIM or from an Apple silicon Mac's internal drive (encrypted, wiped by the Mac); on phones only copies that still exist (trash folders, chat apps, previews, backups) can come back; Intel Macs are not supported.

## Download

- [Download for Mac and Windows](${SITE}/): official site; installers also on [GitHub Releases](https://github.com/trevnedai/recovme/releases/latest) with SHA256SUMS.txt
- [What's new](https://github.com/trevnedai/recovme/blob/main/CHANGELOG.md)
- [Source code](https://github.com/trevnedai/recovme)

## Guides

${lines("guide", "en")}

## Comparisons

${lines("compare", "en")}

## About

- [About RecovMe and its maker](${SITE}${page.about.en.path})
- [Privacy](${SITE}${page.privacy.en.path}): no analytics, no tracking, files never leave the computer
- [Press kit](${SITE}/press/)

## Italiano

- [RecovMe in italiano](${SITE}/it/)
${guides.map((g) => `- [${g.it.h1}](${url("it", g.it.slug)})`).join("\n")}
`);
}

console.log(`Built ${guides.length * 2} guide pages, ${pages.length * 2} pages, lists, press kit, Italian home, sitemap.xml and llms.txt`);
