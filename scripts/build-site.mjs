// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
// Builds the generated parts of recovme.app.
//
//   node scripts/build-site.mjs      (or: npm run site)
//
// Reads site-src/ and writes into site/:
//   guides/<slug>/, it/guide/<slug>/…   guides and comparisons (site-src/guides/pages.json
//                                       + one body file per page and language; a page is
//                                       built only in the languages it has)
//   guides/, it/guide/, es/guias/…      lists of guides and comparisons
//   about/, privacy/, terms/ and their translations (site-src/pages/pages.json)
//   press/                              press kit (site-src/press.html)
//   it/index.html, es/index.html…       translated home pages, built from site/index.html
//                                       with site-src/home.<lang>.json
//   language links and structured data on every home page, sitemap.xml and llms.txt
// The generated files are committed, because GitHub Pages serves site/ as-is.
// To add a language: add it to LANGS, NAMES and L, then add site-src/home.<lang>.json
// and any guides (an entry in pages.json plus <id>.<lang>.html).

import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SITE = "https://recovme.app";
const src = (...p) => join(root, "site-src", ...p);
const out = (...p) => join(root, "site", ...p);
const { updated, guides } = JSON.parse(readFileSync(src("guides", "pages.json"), "utf8"));
const { pages } = JSON.parse(readFileSync(src("pages", "pages.json"), "utf8"));
const LANGS = ["en", "it", "es", "pt", "de", "fr"];
const NAMES = { en: "English", it: "Italiano", es: "Español", pt: "Português", de: "Deutsch", fr: "Français" };

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const strip = (s) => s.replace(/<[^>]+>/g, "").replace(/\s+/g, " ").trim()
  .replace(/&amp;/g, "&").replace(/&nbsp;/g, " ").replace(/&rsquo;/g, "’");
const write = (file, html) => { mkdirSync(dirname(file), { recursive: true }); writeFileSync(file, html); };
const pathOut = (p) => out(...p.split("/").filter(Boolean), "index.html");

const L = {
  en: {
    root: "/", hub: "/guides/", hubName: "Guides", locale: "en_US", language: "Language",
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
    root: "/it/", hub: "/it/guide/", hubName: "Guide", locale: "it_IT", language: "Lingua",
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
  es: {
    root: "/es/", hub: "/es/guias/", hubName: "Guías", locale: "es_ES", language: "Idioma",
    home: "Inicio", questions: "Preguntas", download: "Descargar",
    by: "Por Dhan Denvert Asuncion", updated: "Actualizada el", platforms: "Mac y Windows",
    date: (d) => new Date(d + "T12:00:00Z").toLocaleDateString("es-ES", { day: "numeric", month: "long", year: "numeric" }),
    getTitle: "Descarga RecovMe. Es gratis.",
    getText: "Sin prueba, sin límite de tamaño y sin pagar después del escaneo. Tus archivos nunca salen de tu ordenador.",
    mac: "Descargar para Mac", win: "Descargar para Windows",
    req: "Mac con Apple silicon, macOS 11 o posterior · Windows 10 u 11, 64 bits",
    more: "Más guías", moreCompare: "Más comparativas", moreLink: "Todas las guías",
    foot: "© 2026 Dhan Denvert Asuncion. Gratis y de código abierto con licencia GPL-3.0.",
    signing: "Política de firma de código", source: "Código fuente en GitHub", press: "Prensa",
    pageNames: { about: "Acerca de", privacy: "Privacidad", terms: "Condiciones" },
    hubTitle: "Guías gratis de recuperación de archivos | RecovMe",
    hubH1: "Guías de recuperación de archivos",
    hubLede: "Ayuda paso a paso para las formas más comunes de perder archivos, con una respuesta honesta sobre tus posibilidades primero. Cada guía usa RecovMe, gratis y sin límites.",
    hubDesc: "Guías gratuitas paso a paso para recuperar fotos, vídeos y archivos borrados de tarjetas SD, memorias USB y discos duros en Mac y Windows.",
    hubGuides: "Guías de recuperación", hubCompare: "Alternativas gratuitas y comparativas",
    read: "Leer la guía", readCompare: "Leer la comparativa",
  },
  pt: {
    root: "/pt/", hub: "/pt/guias/", hubName: "Guias", locale: "pt_BR", language: "Idioma",
    home: "Início", questions: "Perguntas", download: "Baixar",
    by: "Por Dhan Denvert Asuncion", updated: "Atualizado em", platforms: "Mac e Windows",
    date: (d) => new Date(d + "T12:00:00Z").toLocaleDateString("pt-BR", { day: "numeric", month: "long", year: "numeric" }),
    getTitle: "Baixe o RecovMe. É grátis.",
    getText: "Sem teste, sem limite de tamanho e nada a pagar depois da varredura. Seus arquivos nunca saem do seu computador.",
    mac: "Baixar para Mac", win: "Baixar para Windows",
    req: "Mac com Apple silicon, macOS 11 ou posterior · Windows 10 ou 11, 64 bits",
    more: "Mais guias", moreCompare: "Mais comparações", moreLink: "Todos os guias",
    foot: "© 2026 Dhan Denvert Asuncion. Grátis e de código aberto sob a licença GPL-3.0.",
    signing: "Política de assinatura de código", source: "Código-fonte no GitHub", press: "Imprensa",
    pageNames: { about: "Sobre", privacy: "Privacidade", terms: "Termos" },
    hubTitle: "Guias grátis de recuperação de arquivos | RecovMe",
    hubH1: "Guias de recuperação de arquivos",
    hubLede: "Ajuda passo a passo para as formas mais comuns de perder arquivos, com uma resposta honesta sobre suas chances primeiro. Todos os guias usam o RecovMe, grátis e sem limites.",
    hubDesc: "Guias gratuitos passo a passo para recuperar fotos, vídeos e arquivos apagados de cartões SD, pen drives e HDs no Mac e no Windows.",
    hubGuides: "Guias de recuperação", hubCompare: "Alternativas gratuitas e comparações",
    read: "Ler o guia", readCompare: "Ler a comparação",
  },
  de: {
    root: "/de/", hub: "/de/anleitungen/", hubName: "Anleitungen", locale: "de_DE", language: "Sprache",
    home: "Start", questions: "Fragen", download: "Herunterladen",
    by: "Von Dhan Denvert Asuncion", updated: "Aktualisiert am", platforms: "Mac und Windows",
    date: (d) => new Date(d + "T12:00:00Z").toLocaleDateString("de-DE", { day: "numeric", month: "long", year: "numeric" }),
    getTitle: "Hol dir RecovMe. Kostenlos.",
    getText: "Keine Testversion, keine Größengrenze, keine Bezahlschranke nach dem Scan. Deine Dateien verlassen nie deinen Computer.",
    mac: "Für Mac herunterladen", win: "Für Windows herunterladen",
    req: "Mac mit Apple silicon, macOS 11 oder neuer · Windows 10 oder 11, 64 Bit",
    more: "Weitere Anleitungen", moreCompare: "Weitere Vergleiche", moreLink: "Alle Anleitungen",
    foot: "© 2026 Dhan Denvert Asuncion. Kostenlos und Open Source unter der GPL-3.0.",
    signing: "Richtlinie zur Code-Signierung", source: "Quellcode auf GitHub", press: "Presse",
    pageNames: { about: "Über RecovMe", privacy: "Datenschutz", terms: "Nutzungsbedingungen" },
    hubTitle: "Kostenlose Anleitungen zur Datenrettung | RecovMe",
    hubH1: "Anleitungen zur Datenrettung",
    hubLede: "Schritt-für-Schritt-Hilfe für die häufigsten Arten, Dateien zu verlieren, mit einer ehrlichen Einschätzung deiner Chancen vorweg. Jede Anleitung nutzt RecovMe, kostenlos und ohne Grenzen.",
    hubDesc: "Kostenlose Schritt-für-Schritt-Anleitungen, um gelöschte Fotos, Videos und Dateien von SD-Karten, USB-Sticks und Festplatten auf Mac und Windows wiederherzustellen.",
    hubGuides: "Anleitungen zur Wiederherstellung", hubCompare: "Kostenlose Alternativen und Vergleiche",
    read: "Anleitung lesen", readCompare: "Vergleich lesen",
  },
  fr: {
    root: "/fr/", hub: "/fr/guides/", hubName: "Guides", locale: "fr_FR", language: "Langue",
    home: "Accueil", questions: "Questions", download: "Télécharger",
    by: "Par Dhan Denvert Asuncion", updated: "Mis à jour le", platforms: "Mac et Windows",
    date: (d) => new Date(d + "T12:00:00Z").toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" }),
    getTitle: "Téléchargez RecovMe. C’est gratuit.",
    getText: "Pas d’essai, pas de limite de taille, rien à payer après l’analyse. Vos fichiers ne quittent jamais votre ordinateur.",
    mac: "Télécharger pour Mac", win: "Télécharger pour Windows",
    req: "Mac avec puce Apple, macOS 11 ou ultérieur · Windows 10 ou 11, 64 bits",
    more: "Plus de guides", moreCompare: "Plus de comparatifs", moreLink: "Tous les guides",
    foot: "© 2026 Dhan Denvert Asuncion. Gratuit et open source sous licence GPL-3.0.",
    signing: "Politique de signature du code", source: "Code source sur GitHub", press: "Presse",
    pageNames: { about: "À propos", privacy: "Confidentialité", terms: "Conditions" },
    hubTitle: "Guides gratuits de récupération de fichiers | RecovMe",
    hubH1: "Guides de récupération de fichiers",
    hubLede: "Une aide pas à pas pour les façons les plus courantes de perdre des fichiers, avec d’abord une réponse honnête sur vos chances. Chaque guide utilise RecovMe, gratuit et sans limite.",
    hubDesc: "Guides gratuits pas à pas pour récupérer des photos, vidéos et fichiers supprimés sur cartes SD, clés USB et disques durs, sur Mac et Windows.",
    hubGuides: "Guides de récupération", hubCompare: "Alternatives gratuites et comparatifs",
    read: "Lire le guide", readCompare: "Lire le comparatif",
  },
};

// The languages a guide or page exists in, and its address in each.
const has = (x) => LANGS.filter((l) => x[l]);

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

// Language menu: links to this page in every language, or to that language's
// home page where the page isn't translated.
function langMenu(lang, links) {
  const items = LANGS.filter((l) => l !== lang)
    .map((l) => `<a href="${links[l] || L[l].root}" hreflang="${l}" lang="${l}">${NAMES[l]}</a>`).join("");
  return `<details class="langs"><summary aria-label="${NAMES[lang]} (${L[lang].language})" data-short="${lang.toUpperCase()}">${NAMES[lang]}</summary><div>${items}</div></details>`;
}

function topbar(lang, links, current) {
  const t = L[lang];
  return `<header class="top">
  <div class="wrap">
    <a class="brand" href="${t.root}"><img src="/assets/icon-256.png" width="36" height="36" alt="">RecovMe</a>
    <nav aria-label="Site">
      <a href="${t.hub}"${current ? ' aria-current="page"' : ""}>${t.hubName}</a>
      <a href="${t.root}#questions">${t.questions}</a>
      ${langMenu(lang, links)}
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
  const p = (id) => page[id][lang]
    ? `<a href="${page[id][lang].path}">${page[id][lang].short}</a>`
    : `<a href="${page[id].en.path}" hreflang="en">${t.pageNames[id]}</a>`;
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
  const alternates = Object.fromEntries(has(g).map((l) => [l, url(l, g[l].slug)]));
  const links = Object.fromEntries(has(g).map((l) => [l, path(l, g[l].slug)]));
  for (const lang of has(g)) {
    const t = L[lang], p = g[lang];
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
    const inLang = guides.filter((x) => x[lang]);
    const same = inLang.filter((x) => x.id !== g.id && kindOf(x) === kindOf(g));
    const others = inLang.filter((x) => kindOf(x) !== kindOf(g)).slice(0, 2);
    const related = [...same, ...others].slice(0, 5).map((x) => card(lang, x)).join("\n");
    const html = head({ lang, title: p.title, desc: p.description, canonical, type: "article", ld, alternates })
      + topbar(lang, links, false) + `
<main>
  <div class="wrap">
    <ol class="crumbs"><li><a href="${t.root}">RecovMe</a></li><li><a href="${t.hub}">${t.hubName}</a></li><li aria-current="page">${esc(p.short)}</li></ol>
    <div class="head">
      <h1>${esc(p.h1)}</h1>
      <p class="lede">${p.lede}</p>
      <p class="meta"><a href="${(page.about[lang] || page.about.en).path}">${t.by}</a><span>${t.updated} ${t.date(date)}</span><span>${t.platforms}</span></p>
    </div>
    <article>
${body}
    </article>
${getBox(lang)}
${related ? `    <section class="related" aria-labelledby="more-title">
      <h2 id="more-title">${kindOf(g) === "compare" ? t.moreCompare : t.more}</h2>
      <ul class="cards">
${related}
      </ul>
      <p><a href="${t.hub}">${t.moreLink} →</a></p>
    </section>
` : ""}  </div>
</main>
` + footer(lang);
    write(out(...t.hub.split("/").filter(Boolean), p.slug, "index.html"), html);
  }
  for (const l of has(g)) sitemap.push([alternates[l], guideDate(g), alternates]);
}

// Lists of guides and comparisons
const newest = guides.map(guideDate).sort().at(-1);
const hubAlts = Object.fromEntries(LANGS.map((l) => [l, url(l)]));
const hubLinks = Object.fromEntries(LANGS.map((l) => [l, L[l].hub]));
for (const lang of LANGS) {
  const t = L[lang];
  const inLang = guides.filter((g) => g[lang]);
  const ld = {
    "@context": "https://schema.org",
    "@graph": [
      { "@type": "CollectionPage", name: t.hubH1, description: t.hubDesc, inLanguage: lang, url: url(lang), about: app },
      { "@type": "ItemList", itemListElement: inLang.map((g, i) => ({ "@type": "ListItem", position: i + 1, url: url(lang, g[lang].slug), name: g[lang].h1 })) },
    ],
  };
  const list = (kind, title) => {
    const items = inLang.filter((g) => kindOf(g) === kind).map((g) => card(lang, g)).join("\n");
    return items ? `    <h2 class="list-title">${title}</h2>
    <ul class="cards">
${items}
    </ul>
` : "";
  };
  const html = head({ lang, title: t.hubTitle, desc: t.hubDesc, canonical: url(lang), type: "website", ld, alternates: hubAlts })
    + topbar(lang, hubLinks, true) + `
<main>
  <div class="wrap">
    <ol class="crumbs"><li><a href="${t.root}">RecovMe</a></li><li aria-current="page">${t.hubName}</li></ol>
    <div class="head">
      <h1>${t.hubH1}</h1>
      <p class="lede">${t.hubLede}</p>
    </div>
${list("guide", t.hubGuides)}${list("compare", t.hubCompare)}${getBox(lang)}
  </div>
</main>
` + footer(lang);
  write(out(...t.hub.split("/").filter(Boolean), "index.html"), html);
  sitemap.push([url(lang), newest, hubAlts]);
}

// About, Privacy and Terms
for (const pg of pages) {
  const alternates = Object.fromEntries(has(pg).map((l) => [l, SITE + pg[l].path]));
  const links = Object.fromEntries(has(pg).map((l) => [l, pg[l].path]));
  for (const lang of has(pg)) {
    const t = L[lang], p = pg[lang];
    const body = readFileSync(src("pages", `${pg.id}.${lang}.html`), "utf8").trim();
    const canonical = SITE + p.path;
    const ld = {
      "@context": "https://schema.org",
      "@graph": [
        { "@type": pg.type || "WebPage", name: p.h1, description: p.description, url: canonical, inLanguage: lang, dateModified: pg.updated,
          ...(pg.id === "about" ? { mainEntity: { "@id": person["@id"] } } : {}), about: app },
        person, org,
      ],
    };
    const html = head({ lang, title: p.title, desc: p.description, canonical, type: "website", ld, alternates })
      + topbar(lang, links, false) + `
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
  for (const l of has(pg)) sitemap.push([alternates[l], pg.updated, alternates]);
}

// Press kit (English, with an Italian paragraph)
{
  const body = readFileSync(src("press.html"), "utf8").trim();
  const canonical = SITE + "/press/";
  const desc = "RecovMe press kit: a short description, key facts, the app icon, a banner and screenshots to use in articles, reviews and videos.";
  const ld = { "@context": "https://schema.org", "@graph": [{ "@type": "WebPage", name: "RecovMe Press Kit", description: desc, url: canonical, inLanguage: "en", about: app }, person, org] };
  const html = head({ lang: "en", title: "Press Kit | RecovMe", desc, canonical, type: "website", ld }) + topbar("en", {}, false) + `
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

// Translated home pages: built from the English one with the snippets in
// site-src/home.<lang>.json. Snippets that no longer match are listed, so a change
// to the English page never slips through untranslated without a warning.
const homeLangs = LANGS.filter((l) => l === "en" || existsSync(src(`home.${l}.json`)));
const homeFile = (l) => l === "en" ? out("index.html") : out(l, "index.html");
{
  const en = readFileSync(out("index.html"), "utf8");
  for (const lang of homeLangs.filter((l) => l !== "en")) {
    const { pairs } = JSON.parse(readFileSync(src(`home.${lang}.json`), "utf8"));
    let html = en;
    const missing = [];
    for (const [from, to] of [...pairs].sort((a, b) => b[0].length - a[0].length)) {
      if (!html.includes(from)) { missing.push(from); continue; }
      html = html.split(from).join(to);
    }
    html = html
      .replace(/(src|href)="assets\//g, '$1="/assets/')
      .replace(/url\(assets\//g, "url(/assets/")
      .replace(/href="code-signing\.html"/g, 'href="/code-signing.html"');
    if (missing.length) {
      console.warn(`\n${NAMES[lang]} home page: ${missing.length} English snippet(s) not found. Update site-src/home.${lang}.json:`);
      for (const m of missing) console.warn("  - " + m.slice(0, 110));
    }
    write(homeFile(lang), html);
  }
}

// Language links on every home page: hreflang tags in the head and a list in the
// footer, between <!--hreflang--> and <!--langs--> markers in site/index.html.
const homeAlts = Object.fromEntries(homeLangs.map((l) => [l, SITE + L[l].root]));
for (const lang of homeLangs) {
  const html = readFileSync(homeFile(lang), "utf8");
  const tags = homeLangs.map((l) => `<link rel="alternate" hreflang="${l}" href="${homeAlts[l]}">`)
    .concat(`<link rel="alternate" hreflang="x-default" href="${homeAlts.en}">`).join("\n");
  const list = homeLangs.map((l) => l === lang
    ? `<b>${NAMES[l]}</b>`
    : `<a href="${L[l].root}" hreflang="${l}" lang="${l}">${NAMES[l]}</a>`).join(" · ");
  for (const m of ["hreflang", "langs"]) {
    if (!html.includes(`<!--${m}-->`)) throw new Error(`${homeFile(lang)} is missing <!--${m}--> … <!--/${m}-->`);
  }
  writeFileSync(homeFile(lang), html
    .replace(/<!--hreflang-->[\s\S]*?<!--\/hreflang-->/, `<!--hreflang-->\n${tags}\n<!--/hreflang-->`)
    .replace(/<!--langs-->[\s\S]*?<!--\/langs-->/, `<!--langs-->${list}<!--/langs-->`));
}

// Home page structured data: the app, its maker and the Questions section,
// rebuilt from each page itself so it never drifts from what visitors see.
for (const lang of homeLangs) {
  const file = homeFile(lang), pageUrl = homeAlts[lang];
  const html = readFileSync(file, "utf8");
  const questions = (html.split('id="questions"')[1] || "").split("</section>")[0];
  const ld = {
    "@context": "https://schema.org",
    "@graph": [
      { "@type": "WebSite", "@id": SITE + "/#site", name: "RecovMe", url: SITE + "/", inLanguage: homeLangs, publisher: { "@id": org["@id"] } },
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
sitemap.unshift(...homeLangs.map((l) => [homeAlts[l], updated, homeAlts]));
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

${LANGS.filter((l) => l !== "en").map((l) => `## ${NAMES[l]}

- [RecovMe (${NAMES[l]})](${SITE}${L[l].root})
${guides.filter((g) => g[l]).map((g) => `- [${g[l].h1}](${url(l, g[l].slug)})`).join("\n")}
`).join("\n")}`);
}

const count = (xs) => xs.reduce((n, x) => n + has(x).length, 0);
console.log(`Built ${count(guides)} guide pages, ${count(pages)} pages, ${LANGS.length} guide lists, press kit, ${homeLangs.length} home pages, sitemap.xml and llms.txt`);
