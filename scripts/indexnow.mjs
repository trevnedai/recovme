// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
// Tells search engines that use IndexNow (Bing, which also feeds Copilot and
// partly ChatGPT, plus Yandex, Naver, Seznam and others) which pages changed.
// Google doesn't use IndexNow; it finds changes through sitemap.xml.
//
//   node scripts/indexnow.mjs site/index.html site/guides/x/index.html   changed files
//   node scripts/indexnow.mjs --all                                      every page in sitemap.xml
//
// The key is public by design: site/<key>.txt proves the site is ours.

import { readFileSync } from "node:fs";

const SITE = "https://recovme.app";
const KEY = "aa96bfbb535c4f67865e02076b1b8c1d";
const args = process.argv.slice(2);

const toUrl = (file) => {
  const m = file.replace(/\\/g, "/").match(/^site\/(.*)$/);
  if (!m || !/\.html$/.test(m[1])) return null;               // pages only, not images or styles
  return SITE + "/" + m[1].replace(/(^|\/)index\.html$/, "$1");
};

let urls = args.includes("--all")
  ? [...readFileSync(new URL("../site/sitemap.xml", import.meta.url), "utf8").matchAll(/<loc>([^<]+)<\/loc>/g)].map((m) => m[1])
  : args.map(toUrl).filter(Boolean);
urls = [...new Set(urls)];
if (!urls.length) { console.log("IndexNow: no changed pages to submit."); process.exit(0); }

const res = await fetch("https://api.indexnow.org/indexnow", {
  method: "POST",
  headers: { "Content-Type": "application/json; charset=utf-8" },
  body: JSON.stringify({ host: "recovme.app", key: KEY, keyLocation: `${SITE}/${KEY}.txt`, urlList: urls }),
});
console.log(`IndexNow: submitted ${urls.length} page(s), answer ${res.status} ${res.statusText}`);
for (const u of urls) console.log("  " + u);
// 200 and 202 mean accepted. Don't fail the deploy over this.
