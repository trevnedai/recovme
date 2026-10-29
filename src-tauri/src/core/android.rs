//! Android phones and tablets over USB, through adb (USB debugging).
//!
//! A phone's storage is encrypted and deleted space is wiped, so it can't be
//! carved like a memory card. What survives, and what adb can read without
//! rooting the phone:
//!   - the system trash: Android 11+ keeps deleted photos and videos for 30 days
//!     as hidden ".trashed-<expiry>-<name>" files next to where they were
//!   - gallery and file manager trash folders (Samsung, Xiaomi, Huawei, OPPO…)
//!   - thumbnail caches, which often still show photos deleted long ago
//!   - chat app media (WhatsApp, Telegram…), kept even after a chat is cleared
//!   - optionally, a copy of everything in the phone's own folders
//!
//! The phone is only read: files are copied off it with `adb pull`.

use super::devices::{clean_part, clean_rel, fix_name, untrash_name, Ctx};
use super::types::Drive;
use super::util::{cmd, output_with_timeout, shq};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const PREFIX: &str = "adb:";
pub const TRASH_FOLDER: &str = "Recently Deleted";
pub const THUMBS_FOLDER: &str = "Thumbnails";
pub const STORAGE_FOLDER: &str = "Phone Storage";

/// (place, folder on the phone, where it goes in the results)
const PLACES: &[(&str, &str, &str)] = &[
    // Trash folders of gallery and file apps. The more specific ones come first.
    ("trash", "Android/.Trash/com.sec.android.gallery3d", "Recently Deleted/Samsung Gallery"),
    ("trash", "Android/.Trash/com.sec.android.app.myfiles", "Recently Deleted/Samsung My Files"),
    ("trash", "Android/.Trash", "Recently Deleted/Trash"),
    ("trash", "Android/data/com.sec.android.gallery3d/files/.Trash", "Recently Deleted/Samsung Gallery"),
    ("trash", "Android/data/com.sec.android.app.myfiles/files/trash", "Recently Deleted/Samsung My Files"),
    ("trash", "MIUI/Gallery/cloud/.trashBin", "Recently Deleted/Xiaomi Gallery"),
    ("trash", "Android/data/com.miui.gallery/files/trashBin", "Recently Deleted/Xiaomi Gallery"),
    ("trash", "Android/data/com.mi.android.globalFileexplorer/files/trash_bin", "Recently Deleted/Xiaomi File Manager"),
    ("trash", "DCIM/.globalTrash", "Recently Deleted/Gallery"),
    ("trash", "Pictures/.Gallery2/recycle", "Recently Deleted/Huawei Gallery"),
    ("trash", "Android/data/com.huawei.photos/files/recycle", "Recently Deleted/Huawei Gallery"),
    ("trash", "Android/data/com.coloros.gallery3d/files/Recycle", "Recently Deleted/OPPO Gallery"),
    ("trash", "Android/data/com.oneplus.gallery/files/recycle", "Recently Deleted/OnePlus Gallery"),
    ("trash", "Android/data/com.vivo.gallery/files/recycle", "Recently Deleted/vivo Gallery"),
    ("trash", "Android/data/com.google.android.apps.nbu.files/files/.trash", "Recently Deleted/Files by Google"),
    ("trash", "DCIM/.trash", "Recently Deleted/Trash"),
    ("trash", "Pictures/.trash", "Recently Deleted/Trash"),
    ("trash", ".Trash", "Recently Deleted/Trash"),
    ("trash", ".trash", "Recently Deleted/Trash"),
    ("trash", ".recycle", "Recently Deleted/Recycle Bin"),
    ("trash", ".RecycleBin", "Recently Deleted/Recycle Bin"),
    // Thumbnail caches.
    ("thumbnails", "DCIM/.thumbnails", "Thumbnails/DCIM"),
    ("thumbnails", "Pictures/.thumbnails", "Thumbnails/Pictures"),
    ("thumbnails", "Movies/.thumbnails", "Thumbnails/Movies"),
    ("thumbnails", "Android/data/com.sec.android.gallery3d/cache", "Thumbnails/Samsung Gallery"),
    // Chat and social apps (new Android/media places first, then the old ones).
    ("apps", "Android/media/com.whatsapp/WhatsApp/Media", "WhatsApp"),
    ("apps", "WhatsApp/Media", "WhatsApp"),
    ("apps", "Android/media/com.whatsapp.w4b/WhatsApp Business/Media", "WhatsApp Business"),
    ("apps", "WhatsApp Business/Media", "WhatsApp Business"),
    ("apps", "Android/data/org.telegram.messenger/files/Telegram", "Telegram"),
    ("apps", "Android/data/org.telegram.messenger.web/files/Telegram", "Telegram"),
    ("apps", "Telegram", "Telegram"),
    ("apps", "Pictures/Telegram", "Telegram/Pictures"),
    ("apps", "Movies/Telegram", "Telegram/Movies"),
    ("apps", "Download/Telegram", "Telegram/Download"),
    ("apps", "Android/media/com.viber.voip/Viber", "Viber"),
    ("apps", "viber/media", "Viber"),
    ("apps", "Pictures/Viber", "Viber"),
    ("apps", "Pictures/Messenger", "Messenger"),
    ("apps", "Pictures/Instagram", "Instagram"),
    ("apps", "Movies/Instagram", "Instagram"),
    ("apps", "Pictures/Signal", "Signal"),
    ("apps", "Android/media/jp.naver.line.android", "LINE"),
    ("apps", "Pictures/WeChat", "WeChat"),
    // Everything in the phone's own folders.
    ("all", "DCIM", "Phone Storage/DCIM"),
    ("all", "Pictures", "Phone Storage/Pictures"),
    ("all", "Movies", "Phone Storage/Movies"),
    ("all", "Music", "Phone Storage/Music"),
    ("all", "Recordings", "Phone Storage/Recordings"),
    ("all", "Download", "Phone Storage/Download"),
    ("all", "Documents", "Phone Storage/Documents"),
    ("all", "Audiobooks", "Phone Storage/Audiobooks"),
    ("all", "Podcasts", "Phone Storage/Podcasts"),
];

fn adb(adb: &Path, serial: Option<&str>) -> Command {
    let mut c = cmd(&adb.to_string_lossy());
    if let Some(s) = serial {
        c.args(["-s", s]);
    }
    c
}

/// Runs a shell script on the phone and returns what it printed.
fn shell(adb_path: &Path, serial: &str, script: &str, limit: Duration) -> Result<String, String> {
    let mut c = adb(adb_path, Some(serial));
    c.args(["shell", script]);
    let out = output_with_timeout(c, limit).map_err(|e| format!("The phone didn't answer ({e})."))?;
    let text = String::from_utf8_lossy(&out.stdout).replace('\r', "");
    if !out.status.success() && text.trim().is_empty() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(friendly_error(&err));
    }
    Ok(text)
}

fn friendly_error(err: &str) -> String {
    let low = err.to_ascii_lowercase();
    if low.contains("unauthorized") {
        "The phone hasn't allowed this computer yet. Unlock it and tap Allow on the “Allow USB debugging?” message.".into()
    } else if low.contains("not found") || low.contains("offline") || low.contains("no devices") {
        "The phone was disconnected. Plug it back in, unlock it, and try again.".into()
    } else if err.is_empty() {
        "The phone couldn't be read.".into()
    } else {
        format!("The phone couldn't be read ({err}).")
    }
}

/// Phones plugged in with USB debugging on, including ones still waiting for
/// the "Allow USB debugging?" tap (shown, but not ready yet).
pub fn list(adb_path: &Path) -> Vec<Drive> {
    let mut c = adb(adb_path, None);
    c.args(["devices", "-l"]);
    // The first call starts adb's background service, which takes a moment.
    let Ok(out) = output_with_timeout(c, Duration::from_secs(20)) else { return vec![] };
    let text = String::from_utf8_lossy(&out.stdout).replace('\r', "");
    let mut phones = Vec::new();
    for line in text.lines().skip_while(|l| !l.starts_with("List of devices")).skip(1) {
        let mut words = line.split_whitespace();
        let (Some(serial), Some(state)) = (words.next(), words.next()) else { continue };
        let rest: Vec<&str> = words.collect();
        let model_hint = rest
            .iter()
            .find_map(|w| w.strip_prefix("model:"))
            .map(|m| m.replace('_', " "));
        let blocked = match state {
            "device" => None,
            "unauthorized" => Some(
                "Unlock the phone and tap Allow on the “Allow USB debugging?” message, then press refresh.".to_string(),
            ),
            "no" => Some(
                "This computer isn't allowed to use the phone over USB. Unplug it, plug it back in, and choose “File transfer” on the phone.".to_string(),
            ),
            "recovery" | "sideload" | "bootloader" => Some("Start the phone normally, then press refresh.".to_string()),
            _ => Some("The phone isn't responding. Unplug it, plug it back in, and unlock it.".to_string()),
        };
        let mut label = model_hint.clone().unwrap_or_else(|| "Android phone".into());
        let mut detail = "Android · USB".to_string();
        let mut has_card = false;
        if blocked.is_none() {
            if let Ok(info) = shell(adb_path, serial, INFO_SCRIPT, Duration::from_secs(10)) {
                let get = |k: &str| {
                    info.lines()
                        .find_map(|l| l.strip_prefix(&format!("{k}=")))
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                };
                let market = get("ro.product.marketname")
                    .or_else(|| get("ro.product.vendor.marketname"))
                    .or_else(|| get("ro.config.marketing_name"));
                let maker = get("ro.product.manufacturer").unwrap_or_default();
                let model = get("ro.product.model").unwrap_or_default();
                label = market.unwrap_or_else(|| {
                    let maker = capitalize(&maker);
                    if model.to_lowercase().starts_with(&maker.to_lowercase()) || maker.is_empty() {
                        model.clone()
                    } else {
                        format!("{maker} {model}")
                    }
                });
                if label.trim().is_empty() {
                    label = model_hint.clone().unwrap_or_else(|| "Android phone".into());
                }
                if let Some(v) = get("ro.build.version.release") {
                    detail = format!("Android {v} · USB");
                }
                has_card = info.lines().any(|l| l.starts_with("card="));
            }
        }
        phones.push(Drive {
            id: format!("{PREFIX}{serial}"),
            label,
            detail: if has_card { format!("{detail} · memory card inside") } else { detail },
            device: format!("{PREFIX}{serial}"),
            kind: "android".into(),
            is_whole_disk: true,
            warning: has_card.then(|| {
                "This phone has a memory card. For the best results, also put the card in a card reader and scan it as a drive: RecovMe can then search it block by block.".to_string()
            }),
            blocked,
            ..Default::default()
        });
    }
    phones
}

const INFO_SCRIPT: &str = "for p in ro.product.marketname ro.product.vendor.marketname ro.config.marketing_name \
ro.product.manufacturer ro.product.model ro.build.version.release; do echo \"$p=$(getprop $p)\"; done; \
for d in /storage/*-*; do [ -d \"$d\" ] && echo \"card=$d\"; done";

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The script that lists, on the phone, every file in the chosen places.
/// Each line: <place index or T>\t<storage root>\t<folder>\t<file>
fn listing_script(places: &[String]) -> String {
    let mut s = String::from(
        "L() { [ -d \"$3\" ] && find \"$3\" -type f 2>/dev/null | while IFS= read -r f; do printf '%s\\t%s\\t%s\\t%s\\n' \"$1\" \"$2\" \"$3\" \"$f\"; done; }\n\
for r in /storage/emulated/0 /storage/*-*; do\n[ -d \"$r\" ] || continue\n",
    );
    if places.iter().any(|p| p == "trash") {
        s.push_str(
            "find \"$r\" -name '.trashed-*' -type f 2>/dev/null | while IFS= read -r f; do printf 'T\\t%s\\t%s\\t%s\\n' \"$r\" \"$r\" \"$f\"; done\n",
        );
    }
    for (i, (place, dir, _)) in PLACES.iter().enumerate() {
        if places.iter().any(|p| p == place) {
            s.push_str(&format!("L {i} \"$r\" \"$r\"/{}\n", shq(dir)));
        }
    }
    s.push_str("done\necho RECOVME-LIST-END\n");
    s
}

struct Planned {
    remote: String,
    dest_dir: PathBuf,
}

fn plan(listing: &str, ctx: &Ctx) -> (Vec<Planned>, u64) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    let mut skipped = 0;
    for line in listing.lines() {
        let mut cols = line.splitn(4, '\t');
        let (Some(tag), Some(root), Some(base), Some(file)) = (cols.next(), cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        // Memory cards and some phones ignore upper/lower case: ".Trash" is ".trash".
        if !seen.insert(file.to_lowercase()) {
            continue;
        }
        let name = file.rsplit('/').next().unwrap_or(file);
        if name == ".nomedia" {
            continue;
        }
        let shown = untrash_name(name).unwrap_or(name);
        if !ctx.wants(shown) {
            skipped += 1;
            continue;
        }
        let Some(rel) = file.strip_prefix(base).map(|r| r.trim_start_matches('/')) else { continue };
        let rel_dir = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let on_card = !root.ends_with("/emulated/0");
        let mut dest = PathBuf::new();
        if tag == "T" {
            // A trashed file goes back under the folder it was deleted from.
            dest.push(TRASH_FOLDER);
            if on_card {
                dest.push("Memory card");
            }
            dest.push(clean_rel(rel_dir));
        } else {
            let Some((_, _, folder)) = tag.parse::<usize>().ok().and_then(|i| PLACES.get(i)) else { continue };
            let mut parts = folder.split('/');
            if let Some(first) = parts.next() {
                dest.push(first);
            }
            if on_card {
                dest.push("Memory card");
            }
            for p in parts {
                dest.push(p);
            }
            dest.push(clean_rel(rel_dir));
        }
        out.push(Planned { remote: file.to_string(), dest_dir: dest });
    }
    (out, skipped)
}

pub fn run(adb_path: &Path, serial: &str, places: &[String], ctx: &mut Ctx) -> Result<(), String> {
    let places: Vec<String> = if places.is_empty() {
        vec!["trash".into(), "thumbnails".into(), "apps".into()]
    } else {
        places.to_vec()
    };
    ctx.status("Looking for deleted and forgotten files on the phone…");
    let listing = shell(adb_path, serial, &listing_script(&places), Duration::from_secs(600))?;
    if !listing.contains("RECOVME-LIST-END") {
        return Err("The phone stopped answering while its files were listed. Keep it unlocked and connected, then try again.".into());
    }
    let (planned, _skipped) = plan(&listing, ctx);
    let total = planned.len() as u64;

    // One `adb pull` per folder (and per batch of files): far faster than one
    // call per file, and names can't collide because they share a folder.
    let mut batches: BTreeMap<(String, PathBuf), Vec<String>> = BTreeMap::new();
    for p in planned {
        let parent = p.remote.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        batches.entry((parent, p.dest_dir)).or_default().push(p.remote);
    }
    let (mut done, mut bytes, mut failed) = (0u64, 0u64, 0u64);
    ctx.progress(0, total, 0);
    'outer: for ((_, dest_rel), files) in batches {
        let dest = ctx.out.join(&dest_rel);
        if fs::create_dir_all(&dest).is_err() {
            failed += files.len() as u64;
            continue;
        }
        let mut chunk: Vec<String> = Vec::new();
        let mut chars = 0;
        let mut chunks = Vec::new();
        for f in files {
            chars += f.len() + 3;
            chunk.push(f);
            if chunk.len() >= 100 || chars > 6000 {
                chunks.push(std::mem::take(&mut chunk));
                chars = 0;
            }
        }
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        for chunk in chunks {
            if ctx.stopped() {
                break 'outer;
            }
            // Two phone folders can share a results folder: pull a name that is
            // already there on its own, so it can't overwrite the first one.
            let (fresh, clashing): (Vec<&String>, Vec<&String>) =
                chunk.iter().partition(|r| !dest.join(remote_name(r)).exists());
            let fresh: Vec<String> = fresh.into_iter().cloned().collect();
            if !fresh.is_empty() {
                pull(adb_path, serial, &fresh, &dest, ctx);
            }
            let mut got = Vec::new();
            for remote in &fresh {
                let local = dest.join(remote_name(remote));
                if local.is_file() {
                    got.push(local);
                } else if !ctx.stopped() {
                    // A batch stops at the first file it can't read: try the rest one by one.
                    match pull_alone(adb_path, serial, remote, &dest, ctx) {
                        Some(l) => got.push(l),
                        None => failed += 1,
                    }
                }
            }
            for remote in clashing {
                if ctx.stopped() {
                    break;
                }
                match pull_alone(adb_path, serial, remote, &dest, ctx) {
                    Some(l) => got.push(l),
                    None => failed += 1,
                }
            }
            for mut local in got {
                let name = local.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                // Names Windows or macOS can't show are made safe.
                let safe = clean_part(&name);
                if safe != name && !name.starts_with(".trashed-") {
                    let to = super::organize::unique(dest.join(&safe));
                    if fs::rename(&local, &to).is_ok() {
                        local = to;
                    }
                }
                let local = fix_name(&local);
                let size = fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
                bytes += size;
                ctx.found(&local, size);
            }
            done += chunk.len() as u64;
            ctx.progress(done, total, bytes);
        }
    }
    ctx.progress(total, total, bytes);

    ctx.notes.push(
        "Phones encrypt their storage and erase deleted space, so they can't be searched block by block like a memory card. \
RecovMe copied the places where deleted and forgotten files survive: trash folders, thumbnail previews and chat app media."
            .into(),
    );
    if failed > 0 {
        ctx.notes.push(format!(
            "{failed} file{} couldn't be copied: the phone didn't allow it, or it was locked or disconnected during the copy.",
            if failed == 1 { "" } else { "s" }
        ));
    }
    ctx.notes.push(
        "Photos backed up to Google Photos stay in its Trash for 60 days: open photos.google.com → Trash to restore them.".into(),
    );
    Ok(())
}

fn remote_name(remote: &str) -> &str {
    remote.rsplit('/').next().unwrap_or(remote)
}

/// Pulls one file through a private folder, then moves it next to the others
/// under a name that is free. Returns where it ended up.
fn pull_alone(adb_path: &Path, serial: &str, remote: &str, dest: &Path, ctx: &Ctx) -> Option<PathBuf> {
    let tmp = dest.join(".recovme-one");
    fs::create_dir_all(&tmp).ok()?;
    pull(adb_path, serial, &[remote.to_string()], &tmp, ctx);
    let got = tmp.join(remote_name(remote));
    let result = if got.is_file() {
        let to = super::organize::unique(dest.join(remote_name(remote)));
        fs::rename(&got, &to).ok().map(|_| to)
    } else {
        None
    };
    let _ = fs::remove_dir_all(&tmp);
    result
}

/// `adb pull -a` (keeps the files' dates) into `dest`. Can be stopped.
fn pull(adb_path: &Path, serial: &str, files: &[String], dest: &Path, ctx: &Ctx) {
    let mut c = adb(adb_path, Some(serial));
    c.arg("pull").arg("-a").args(files).arg(dest);
    c.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let Ok(mut child) = c.spawn() else { return };
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) if ctx.stopped() => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}
