// Builds the guide pages and sitemap for recovme.app.
//
//   node scripts/build-site.mjs      (or: npm run site)
//
// Reads  site-src/guides/pages.json  and one body file per guide and language
// (site-src/guides/<id>.<lang>.html), then writes:
//   site/guides/<slug>/index.html      English guides
//   site/it/guide/<slug>/index.html    Italian guides
//   site/guides/index.html, site/it/guide/index.html   guide lists
//   site/sitemap.xml
// The generated files are committed, because GitHub Pages serves site/ as-is.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SITE = "https://recovme.app";
const src = (...p) => join(root, "site-src", ...p);
const out = (...p) => join(root, "site", ...p);
const { updated, guides } = JSON.parse(readFileSync(src("guides", "pages.json"), "utf8"));

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const strip = (s) => s.replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim()
  .replace(/&amp;/g, "&").replace(/&nbsp;/g, " ").replace(/&rsquo;/g, "’");
const write = (file, html) => { mkdirSync(dirname(file), { recursive: true }); writeFileSync(file, html); };

const L = {
  en: {
    hub: "/guides/", hubName: "Guides", locale: "en_US", other: "it", otherName: "Italiano",
    home: "Home", questions: "Questions", download: "Download",
    by: "By Dhan Denvert Asuncion", updated: "Updated", platforms: "Mac and Windows",
    date: (d) => new Date(d + "T12:00:00Z").toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric" }),
    getTitle: "Get RecovMe. It’s free.",
    getText: "No trial, no size limit, no paywall after the scan. Nothing leaves your computer.",
    mac: "Download for Mac", win: "Download for Windows",
    req: "Mac with Apple silicon, macOS 11 or later · Windows 10 or 11, 64-bit",
    more: "More guides", moreLink: "All guides",
    foot: "© 2026 Dhan Denvert Asuncion. Free and open source under GPL-3.0.",
    signing: "Code Signing Policy", source: "Source Code on GitHub",
    hubTitle: "File Recovery Guides | RecovMe",
    hubH1: "File Recovery Guides",
    hubLede: "Step-by-step help for the most common ways files get lost, with an honest answer about your chances first. Every guide uses RecovMe, which is free with no limits.",
    hubDesc: "Free step-by-step guides to recover deleted photos, videos and files from SD cards, USB sticks and hard drives on Mac and Windows.",
    read: "Read the guide",
  },
  it: {
    hub: "/it/guide/", hubName: "Guide", locale: "it_IT", other: "en", otherName: "English",
    home: "Home", questions: "Domande", download: "Scarica",
    by: "Di Dhan Denvert Asuncion", updated: "Aggiornata il", platforms: "Mac e Windows",
    date: (d) => new Date(d + "T12:00:00Z").toLocaleDateString("it-IT", { day: "numeric", month: "long", year: "numeric" }),
    getTitle: "Scarica RecovMe. È gratis.",
    getText: "Nessuna prova, nessun limite di dimensione, niente da pagare dopo la scansione. I tuoi file non lasciano mai il computer.",
    mac: "Scarica per Mac", win: "Scarica per Windows",
    req: "Mac con Apple silicon, macOS 11 o successivo · Windows 10 o 11, 64 bit",
    more: "Altre guide", moreLink: "Tutte le guide",
    foot: "© 2026 Dhan Denvert Asuncion. Gratis e open source con licenza GPL-3.0.",
    signing: "Firma del codice", source: "Codice sorgente su GitHub",
    hubTitle: "Guide al recupero dati gratis | RecovMe",
    hubH1: "Guide al recupero dei file",
    hubLede: "Aiuto passo passo per i modi più comuni in cui si perdono i file, con prima una risposta onesta sulle probabilità di recupero. Ogni guida usa RecovMe, gratis e senza limiti.",
    hubDesc: "Guide gratuite passo passo per recuperare foto, video e file cancellati da schede SD, chiavette USB e hard disk su Mac e Windows.",
    read: "Leggi la guida",
  },
};

const url = (lang, slug) => `${SITE}${L[lang].hub}${slug ? slug + "/" : ""}`;
const path = (lang, slug) => `${L[lang].hub}${slug ? slug + "/" : ""}`;

const ICON_MAC = '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M16.4 12.6c0-2.3 1.9-3.4 2-3.5-1.1-1.6-2.8-1.8-3.4-1.8-1.4-.1-2.8.9-3.5.9-.7 0-1.8-.8-3-.8-1.5 0-3 .9-3.8 2.3-1.6 2.8-.4 7 1.2 9.3.8 1.1 1.7 2.4 2.9 2.3 1.2 0 1.6-.7 3-.7s1.8.7 3 .7c1.3 0 2.1-1.1 2.8-2.3.9-1.3 1.3-2.6 1.3-2.6s-2.5-1-2.5-3.8ZM14.1 5.9c.6-.8 1.1-1.8 1-2.9-.9 0-2.1.6-2.7 1.4-.6.7-1.1 1.8-1 2.8 1 .1 2.1-.5 2.7-1.3Z"/></svg>';
const ICON_WIN = '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M3 5.5 10 4.5v7H3zm8-1.1L21 3v8.5H11zM3 12.5h7v7L3 18.5zm8 0h10V21l-10-1.4z"/></svg>';
const RELEASES = "https://github.com/trevnedai/recovme/releases/latest";

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
<link rel="canonical" href="${canonical}">
${alt}
<link rel="icon" href="/assets/favicon.png">
<meta property="og:site_name" content="RecovMe">
<meta property="og:title" content="${esc(title)}">
<meta property="og:description" content="${esc(desc)}">
<meta property="og:image" content="${SITE}/assets/og.png">
<meta property="og:url" content="${canonical}">
<meta property="og:type" content="${type}">
<meta property="og:locale" content="${L[lang].locale}">
<meta name="twitter:card" content="summary_large_image">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,800&family=Instrument+Sans:wght@400;600;700&display=swap">
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
    <a class="brand" href="/"><img src="/assets/icon-256.png" width="36" height="36" alt="">RecovMe</a>
    <nav aria-label="Site">
      <a href="${t.hub}"${current ? ' aria-current="page"' : ""}>${t.hubName}</a>
      <a href="/#questions">${t.questions}</a>
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
  return `<footer class="foot">
  <div class="wrap">
    <p>${t.foot}</p>
    <p><a href="/">${t.home}</a> · <a href="${t.hub}">${t.hubName}</a> · <a href="/code-signing.html">${t.signing}</a> · <a href="https://github.com/trevnedai/recovme" rel="noopener">${t.source}</a></p>
  </div>
</footer>
<script src="/assets/guide.js" defer></script>
</body>
</html>
`;
}

const card = (lang, g) => `<li><a href="${path(lang, g[lang].slug)}"><b>${esc(g[lang].h1)}</b><span>${esc(g[lang].card)}</span><span class="tag">${L[lang].read} →</span></a></li>`;

// Questions and answers in a guide's FAQ, for FAQPage structured data.
function faqs(body) {
  const sec = body.split('class="faq"')[1] || "";
  return [...sec.matchAll(/<summary>([\s\S]*?)<\/summary>\s*<p>([\s\S]*?)<\/p>/g)]
    .map(([, q, a]) => ({ "@type": "Question", name: strip(q), acceptedAnswer: { "@type": "Answer", text: strip(a) } }));
}

const author = { "@type": "Person", name: "Dhan Denvert Asuncion", url: SITE + "/#about" };
const app = { "@type": "SoftwareApplication", "@id": SITE + "/#app", name: "RecovMe", url: SITE + "/" };

for (const g of guides) {
  for (const lang of ["en", "it"]) {
    const t = L[lang], p = g[lang], other = t.other;
    const body = readFileSync(src("guides", `${g.id}.${lang}.html`), "utf8").trim();
    const canonical = url(lang, p.slug);
    const q = faqs(body);
    const ld = {
      "@context": "https://schema.org",
      "@graph": [
        {
          "@type": "Article", headline: p.h1, description: p.description, inLanguage: lang,
          datePublished: g.published, dateModified: updated, author, publisher: author,
          image: SITE + "/assets/og.png", mainEntityOfPage: canonical, about: app,
        },
        {
          "@type": "BreadcrumbList", itemListElement: [
            { "@type": "ListItem", position: 1, name: "RecovMe", item: SITE + "/" },
            { "@type": "ListItem", position: 2, name: t.hubName, item: url(lang) },
            { "@type": "ListItem", position: 3, name: p.short, item: canonical },
          ],
        },
        ...(q.length ? [{ "@type": "FAQPage", mainEntity: q }] : []),
      ],
    };
    const related = guides.filter((x) => x.id !== g.id).map((x) => card(lang, x)).join("\n");
    const html = head({
      lang, title: p.title, desc: p.description, canonical, type: "article", ld,
      alternates: { en: url("en", g.en.slug), it: url("it", g.it.slug) },
    }) + topbar(lang, path(other, g[other].slug), false) + `
<main>
  <div class="wrap">
    <ol class="crumbs"><li><a href="/">RecovMe</a></li><li><a href="${t.hub}">${t.hubName}</a></li><li aria-current="page">${esc(p.short)}</li></ol>
    <div class="head">
      <h1>${esc(p.h1)}</h1>
      <p class="lede">${p.lede}</p>
      <p class="meta"><span>${t.by}</span><span>${t.updated} ${t.date(updated)}</span><span>${t.platforms}</span></p>
    </div>
    <article>
${body}
    </article>
${getBox(lang)}
    <section class="related" aria-labelledby="more-title">
      <h2 id="more-title">${t.more}</h2>
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
}

// Guide lists
for (const lang of ["en", "it"]) {
  const t = L[lang];
  const ld = {
    "@context": "https://schema.org",
    "@graph": [
      { "@type": "CollectionPage", name: t.hubH1, description: t.hubDesc, inLanguage: lang, url: url(lang), about: app },
      { "@type": "ItemList", itemListElement: guides.map((g, i) => ({ "@type": "ListItem", position: i + 1, url: url(lang, g[lang].slug), name: g[lang].h1 })) },
    ],
  };
  const html = head({
    lang, title: t.hubTitle, desc: t.hubDesc, canonical: url(lang), type: "website", ld,
    alternates: { en: url("en"), it: url("it") },
  }) + topbar(lang, L[t.other].hub, true) + `
<main>
  <div class="wrap">
    <ol class="crumbs"><li><a href="/">RecovMe</a></li><li aria-current="page">${t.hubName}</li></ol>
    <div class="head">
      <h1>${t.hubH1}</h1>
      <p class="lede">${t.hubLede}</p>
    </div>
    <ul class="cards">
${guides.map((g) => card(lang, g)).join("\n")}
    </ul>
${getBox(lang)}
  </div>
</main>
` + footer(lang);
  write(out(...t.hub.split("/").filter(Boolean), "index.html"), html);
}

// Home page structured data: the app, its maker and the Questions section,
// rebuilt from the page itself so it never drifts from what visitors see.
{
  const file = out("index.html");
  const page = readFileSync(file, "utf8");
  const questions = (page.split('id="questions"')[1] || "").split("</section>")[0];
  const person = { "@type": "Person", "@id": SITE + "/#maker", name: "Dhan Denvert Asuncion", url: SITE + "/#about" };
  const ld = {
    "@context": "https://schema.org",
    "@graph": [
      { "@type": "WebSite", "@id": SITE + "/#site", name: "RecovMe", url: SITE + "/", inLanguage: "en", publisher: { "@id": person["@id"] } },
      person,
      {
        "@type": "SoftwareApplication", "@id": app["@id"], name: "RecovMe", url: SITE + "/",
        description: strip(page.match(/<meta name="description" content="([^"]+)"/)[1]),
        applicationCategory: "UtilitiesApplication", applicationSubCategory: "Data recovery",
        operatingSystem: "macOS 11 or later (Apple silicon), Windows 10, Windows 11",
        offers: { "@type": "Offer", price: "0", priceCurrency: "EUR" },
        isAccessibleForFree: true,
        license: "https://www.gnu.org/licenses/gpl-3.0.html",
        downloadUrl: RELEASES,
        screenshot: ["app-setup.webp", "app-scan.webp", "app-done.webp"].map((f) => `${SITE}/assets/${f}`),
        image: SITE + "/assets/icon-256.png",
        author: { "@id": person["@id"] },
        codeRepository: "https://github.com/trevnedai/recovme",
      },
      { "@type": "FAQPage", "@id": SITE + "/#questions", mainEntity: faqs('class="faq"' + questions) },
    ],
  };
  const tag = /<script type="application\/ld\+json" id="ld">[\s\S]*?<\/script>/;
  if (!tag.test(page)) throw new Error("site/index.html is missing <script type=\"application/ld+json\" id=\"ld\">");
  writeFileSync(file, page.replace(tag, `<script type="application/ld+json" id="ld">\n${JSON.stringify(ld, null, 1)}\n</script>`));
}

// Sitemap
const entry = (loc, alts) => `  <url>
    <loc>${loc}</loc>
    <lastmod>${updated}</lastmod>${alts ? "\n" + Object.entries(alts).map(([l, u]) => `    <xhtml:link rel="alternate" hreflang="${l}" href="${u}"/>`).join("\n") : ""}
  </url>`;
const pairs = [[url("en"), url("it")], ...guides.map((g) => [url("en", g.en.slug), url("it", g.it.slug)])];
const sitemap = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${entry(SITE + "/")}
${pairs.flatMap(([en, it]) => { const a = { en, it, "x-default": en }; return [entry(en, a), entry(it, a)]; }).join("\n")}
${entry(SITE + "/code-signing.html")}
</urlset>
`;
write(out("sitemap.xml"), sitemap);
console.log(`Built ${guides.length * 2} guides, 2 guide lists and sitemap.xml`);
