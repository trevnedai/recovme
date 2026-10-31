// Checks RecovMe's screens in its built-in demo mode (no Tauri needed).
// Uses Playwright with Chromium: node ui-check.js <path to repo> <folder for screenshots>
const { chromium } = require("playwright");
const [repo, out] = process.argv.slice(2);
const page = (q) => `file://${repo}/src/index.html?demo${q || ""}`;
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1120, height: 780 } });
  const errors = [];
  p.on("pageerror", (e) => errors.push(e.message));
  const check = (name, ok) => console.log(`${ok ? "PASS" : "FAIL"}  ${name}`);

  await p.goto(page());
  await p.waitForTimeout(800);
  await p.screenshot({ path: `${out}/1-welcome.png` });

  // Android: the "Before You Start" dialog must appear; Cancel keeps you out.
  await p.click('.drv[data-id="adb:R58N"]');
  await p.waitForTimeout(400);
  check("Android dialog shown", await p.isVisible("#expectModal"));
  await p.screenshot({ path: `${out}/2-android-dialog.png` });
  await p.click("#expectCancel");
  await p.waitForTimeout(200);
  check("Cancel does not open the phone", !(await p.isVisible("#viewSetup")));
  await p.click('.drv[data-id="adb:R58N"]');
  await p.waitForTimeout(300);
  await p.click("#expectOk");
  await p.waitForTimeout(400);
  check("Android setup: Look In shown", await p.isVisible("#placesBlock"));
  check("Android setup: scan modes hidden", !(await p.isVisible("#modesBlock")));
  check("Button says Recover Files", (await p.textContent("#btnStart")) === "Recover Files");
  await p.screenshot({ path: `${out}/3-android-setup.png` });
  await p.click('.drv[data-id="disk4s1"]');
  await p.waitForTimeout(300);
  await p.click('.drv[data-id="adb:R58N"]');
  await p.waitForTimeout(300);
  check("Dialog shown only once per session", !(await p.isVisible("#expectModal")));

  // Encrypted iPhone backup: needs its password before starting.
  await p.click('.drv[data-id="iosbackup:/x"]');
  await p.waitForTimeout(400);
  await p.click("#expectOk");
  await p.waitForTimeout(400);
  check("Password field shown", await p.isVisible("#passwordBlock"));
  check("Start disabled without password", await p.isDisabled("#btnStart"));
  await p.fill("#backupPassword", "pw");
  await p.waitForTimeout(200);
  check("Start enabled with password", !(await p.isDisabled("#btnStart")));
  await p.screenshot({ path: `${out}/4-iphone-setup.png` });

  // Help dialog.
  await p.click("#btnPhoneHelp");
  await p.waitForTimeout(300);
  check("Phone help opens", await p.isVisible("#phoneModal"));
  await p.keyboard.press("Escape");

  // Update bar (demo pretends 0.3.1 > 0.3.0 is available).
  await p.goto(page("&update"));
  await p.waitForTimeout(3200);
  check("Update bar shown", await p.isVisible("#updateBar"));
  check("What's New link shown", await p.isVisible("#updateNotes"));
  await p.screenshot({ path: `${out}/5-update-bar.png` });
  await p.click("#btnUpdates");
  await p.waitForTimeout(400);
  await p.click("#optAutoUpdate + .switch");
  check("Auto-update off is remembered", (await p.evaluate(() => localStorage.getItem("recovme.autoUpdate"))) === "off");

  check("No page errors", errors.length === 0);
  if (errors.length) console.log(errors);
  await b.close();
})();
