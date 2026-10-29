//! iPhone and iPad backups made by Finder, iTunes or Apple Devices on this computer.
//!
//! Apple never lets another program read an iPhone's storage directly, so a
//! backup is the way in. It holds the Camera Roll (Recently Deleted photos
//! included, until they are purged), Messages attachments, voice memos and the
//! files of apps like WhatsApp. RecovMe reads it without changing anything:
//!   Info.plist      which device, when it was backed up
//!   Manifest.plist  whether it's encrypted, and its keys
//!   Manifest.db     every file: domain, path, size, date (and key, if encrypted)
//!   ab/abcdef…      the files themselves, named by the hash of their path
//! Encrypted backups are opened with the backup password (Apple's keybag:
//! PBKDF2, then AES key wrap per protection class, then AES-256-CBC per file).

use super::devices::{clean_part, clean_rel, Ctx};
use super::types::Drive;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub const PREFIX: &str = "iosbackup:";
pub const CAMERA_ROLL: &str = "Camera Roll";

/// Where Finder, iTunes and the Apple Devices app keep backups.
fn backup_roots() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if let Ok(extra) = std::env::var("RECOVME_IOS_BACKUPS") {
        v.extend(std::env::split_paths(&extra));
    }
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA") {
            v.push(PathBuf::from(a).join("Apple Computer").join("MobileSync").join("Backup"));
        }
        if let Some(u) = std::env::var_os("USERPROFILE") {
            // iTunes from the Microsoft Store and the Apple Devices app.
            v.push(PathBuf::from(&u).join("Apple").join("MobileSync").join("Backup"));
            v.push(PathBuf::from(&u).join("Apple Computer").join("MobileSync").join("Backup"));
        }
    } else if let Some(h) = std::env::var_os("HOME") {
        v.push(
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("MobileSync")
                .join("Backup"),
        );
    }
    v
}

pub fn list() -> Vec<Drive> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for root in backup_roots() {
        let entries = match fs::read_dir(&root) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                if cfg!(target_os = "macos") {
                    out.push(Drive {
                        id: format!("{PREFIX}denied"),
                        label: "iPhone & iPad Backups".into(),
                        detail: "Needs Full Disk Access".into(),
                        kind: "ios-backup".into(),
                        blocked: Some(
                            "macOS keeps iPhone backups private. Allow RecovMe in System Settings → Privacy & Security → Full Disk Access, then reopen RecovMe."
                                .into(),
                        ),
                        ..Default::default()
                    });
                }
                continue;
            }
            Err(_) => continue,
        };
        let mut found = Vec::new();
        for e in entries.flatten() {
            let dir = e.path();
            let key = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
            if dir.is_dir() && seen.insert(key) {
                if let Ok(d) = describe(&dir) {
                    found.push(d);
                }
            }
        }
        // Newest backup first.
        found.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.label.cmp(&b.0.label)));
        out.extend(found.into_iter().map(|(d, _)| d));
    }
    out
}

/// A file picked by hand (on an external drive, say): the backup folder it belongs to.
pub fn backup_dir_of(path: &Path) -> Option<PathBuf> {
    let dir = if path.is_dir() { path.to_path_buf() } else { path.parent()?.to_path_buf() };
    (dir.join("Manifest.plist").is_file() && (dir.join("Manifest.db").is_file() || dir.join("Manifest.mbdb").is_file()))
        .then_some(dir)
}

fn plist_dict(p: &Path) -> Option<plist::Dictionary> {
    plist::Value::from_file(p).ok()?.into_dictionary()
}

fn pstr(d: &plist::Dictionary, k: &str) -> Option<String> {
    d.get(k).and_then(|v| v.as_string()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn drive_for(dir: &Path) -> Result<Drive, String> {
    describe(dir).map(|(d, _)| d)
}

/// The backup as a source to pick, and when it was made (for sorting).
fn describe(dir: &Path) -> Result<(Drive, i64), String> {
    let manifest = plist_dict(&dir.join("Manifest.plist")).ok_or("This folder isn't an iPhone backup.")?;
    let info = plist_dict(&dir.join("Info.plist")).unwrap_or_default();
    let product = pstr(&info, "Product Type").unwrap_or_default();
    let kind = if product.starts_with("iPad") {
        "iPad"
    } else if product.starts_with("iPod") {
        "iPod touch"
    } else {
        "iPhone"
    };
    let name = pstr(&info, "Device Name").or_else(|| pstr(&info, "Display Name")).unwrap_or_else(|| kind.to_string());
    let encrypted = manifest.get("IsEncrypted").and_then(|v| v.as_boolean()).unwrap_or(false);
    let when = info
        .get("Last Backup Date")
        .and_then(|v| v.as_date())
        .map(|d| chrono::DateTime::<chrono::Local>::from(std::time::SystemTime::from(d)));
    let version = pstr(&info, "Product Version");
    let mut bits = vec![format!("{kind} backup")];
    if let Some(v) = version {
        bits.push(format!("iOS {v}"));
    }
    if let Some(w) = when {
        bits.push(w.format("%-d %b %Y").to_string());
    }
    if encrypted {
        bits.push("Encrypted".into());
    }
    let old = !dir.join("Manifest.db").is_file();
    let path = dir.to_string_lossy().to_string();
    let drive = Drive {
        id: format!("{PREFIX}{path}"),
        label: name,
        detail: bits.join(" · "),
        device: format!("{PREFIX}{path}"),
        kind: "ios-backup".into(),
        // Saving into the backup folder is refused like saving onto the drive being scanned.
        mount_points: vec![path],
        is_whole_disk: true,
        encrypted,
        blocked: old.then(|| "This backup is from iOS 9 or older, which RecovMe can't read. Make a new backup in Finder or iTunes.".into()),
        ..Default::default()
    };
    Ok((drive, when.map(|w| w.timestamp()).unwrap_or(0)))
}

// ───────────── encrypted backups ─────────────

type Key = [u8; 32];

#[derive(Default)]
struct ClassKey {
    wrap: u32,
    wpky: Vec<u8>,
    key: Option<Key>,
}

#[derive(Default)]
struct Keybag {
    attrs: HashMap<String, Vec<u8>>,
    classes: HashMap<u32, ClassKey>,
}

const WRAP_PASSCODE: u32 = 2;

fn be32(v: &[u8]) -> u32 {
    v.iter().take(4).fold(0, |a, b| (a << 8) | *b as u32)
}

impl Keybag {
    /// Tag-length-value blocks. The first UUID and WRAP describe the keybag;
    /// every later UUID starts a class key.
    fn parse(data: &[u8]) -> Keybag {
        let mut kb = Keybag::default();
        let mut cur: Option<(u32, ClassKey)> = None;
        let (mut have_uuid, mut have_wrap) = (false, false);
        let mut i = 0;
        while i + 8 <= data.len() {
            let tag = String::from_utf8_lossy(&data[i..i + 4]).to_string();
            let len = be32(&data[i + 4..i + 8]) as usize;
            let Some(val) = data.get(i + 8..i + 8 + len) else { break };
            i += 8 + len;
            match tag.as_str() {
                "UUID" if !have_uuid => {
                    have_uuid = true;
                    kb.attrs.insert(tag, val.to_vec());
                }
                "WRAP" if !have_wrap => {
                    have_wrap = true;
                    kb.attrs.insert(tag, val.to_vec());
                }
                "UUID" => {
                    if let Some((c, k)) = cur.take() {
                        kb.classes.insert(c, k);
                    }
                    cur = Some((0, ClassKey::default()));
                }
                "CLAS" | "WRAP" | "WPKY" | "KTYP" => {
                    if let Some((c, k)) = cur.as_mut() {
                        match tag.as_str() {
                            "CLAS" => *c = be32(val),
                            "WRAP" => k.wrap = be32(val),
                            "WPKY" => k.wpky = val.to_vec(),
                            _ => {}
                        }
                    }
                }
                _ => {
                    kb.attrs.insert(tag, val.to_vec());
                }
            }
        }
        if let Some((c, k)) = cur {
            kb.classes.insert(c, k);
        }
        kb
    }

    fn unlock(&mut self, password: &str) -> Result<(), String> {
        use pbkdf2::pbkdf2_hmac;
        let salt = self.attrs.get("SALT").ok_or("The backup's keys are damaged.")?.clone();
        let iter = be32(self.attrs.get("ITER").ok_or("The backup's keys are damaged.")?);
        // Real backups use 10 million rounds at most; a damaged or doctored file
        // must not keep the app busy for hours.
        const MAX_ROUNDS: u32 = 50_000_000;
        let dpic_ok = self.attrs.get("DPIC").map(|d| be32(d) <= MAX_ROUNDS).unwrap_or(true);
        if iter == 0 || iter > MAX_ROUNDS || !dpic_ok {
            return Err("The backup's keys are damaged.".into());
        }
        let mut pass: Vec<u8> = password.as_bytes().to_vec();
        // iOS 10.2 and later add a SHA-256 round before the SHA-1 one.
        if let (Some(dpsl), Some(dpic)) = (self.attrs.get("DPSL"), self.attrs.get("DPIC")) {
            let mut k = [0u8; 32];
            pbkdf2_hmac::<sha2::Sha256>(&pass, dpsl, be32(dpic), &mut k);
            pass = k.to_vec();
        }
        let mut pk = [0u8; 32];
        pbkdf2_hmac::<sha1::Sha1>(&pass, &salt, iter, &mut pk);
        let mut any = false;
        for ck in self.classes.values_mut() {
            if ck.wpky.is_empty() || ck.wrap & WRAP_PASSCODE == 0 {
                continue;
            }
            let k = aes_unwrap(&pk, &ck.wpky).ok_or("wrong password")?;
            ck.key = Some(k);
            any = true;
        }
        if any {
            Ok(())
        } else {
            Err("The backup's keys are damaged.".into())
        }
    }

    /// A file or manifest key, unwrapped with the key of its protection class.
    fn unwrap(&self, class: u32, wrapped: &[u8]) -> Option<Key> {
        let ck = self.classes.get(&class)?.key?;
        aes_unwrap(&ck, wrapped)
    }
}

fn aes_unwrap(kek: &Key, wrapped: &[u8]) -> Option<Key> {
    let kw = aes_kw::KekAes256::from(*kek);
    let v = kw.unwrap_vec(wrapped).ok()?;
    v.try_into().ok()
}

/// AES-256-CBC with a zero IV, the way backups store files. Streams, so big
/// videos don't have to fit in memory. Removes the padding. The size in
/// Manifest.db is not used: it is often wrong for databases and other files
/// that were changing while the backup ran, and cutting to it damages them.
fn decrypt_file(src: &Path, dst: &Path, key: &Key) -> std::io::Result<u64> {
    use aes::cipher::{generic_array::GenericArray, BlockDecryptMut, KeyIvInit};
    let mut dec = cbc::Decryptor::<aes::Aes256>::new(key.into(), &[0u8; 16].into());
    let mut r = BufReader::with_capacity(1 << 20, fs::File::open(src)?);
    let mut w = BufWriter::with_capacity(1 << 20, fs::File::create(dst)?);
    let mut buf = vec![0u8; 1 << 20];
    let mut held: Option<[u8; 16]> = None;
    let mut written = 0u64;
    loop {
        let mut n = 0;
        while n < buf.len() {
            let got = r.read(&mut buf[n..])?;
            if got == 0 {
                break;
            }
            n += got;
        }
        if n == 0 {
            break;
        }
        let n = n - n % 16;
        for block in buf[..n].chunks_exact_mut(16) {
            dec.decrypt_block_mut(GenericArray::from_mut_slice(block));
        }
        if n == 0 {
            break;
        }
        if let Some(h) = held.take() {
            w.write_all(&h)?;
            written += 16;
        }
        w.write_all(&buf[..n - 16])?;
        written += (n - 16) as u64;
        let mut last = [0u8; 16];
        last.copy_from_slice(&buf[n - 16..n]);
        held = Some(last);
    }
    if let Some(last) = held {
        let pad = last[15] as usize;
        let keep = if (1..=16).contains(&pad) && last[16 - pad..].iter().all(|&b| b as usize == pad) { 16 - pad } else { 16 };
        w.write_all(&last[..keep])?;
        written += keep as u64;
    }
    w.flush()?;
    Ok(written)
}

// ───────────── file records ─────────────

/// What Manifest.db says about one file (an NSKeyedArchiver plist).
#[derive(Default, Debug)]
struct Record {
    size: Option<u64>,
    modified: Option<i64>,
    class: u32,
    key: Option<Vec<u8>>,
}

fn record(blob: &[u8]) -> Record {
    let mut r = Record::default();
    let Ok(v) = plist::Value::from_reader(std::io::Cursor::new(blob)) else { return r };
    let Some(d) = v.as_dictionary() else { return r };
    let Some(objs) = d.get("$objects").and_then(|o| o.as_array()) else { return r };
    let deref = |v: &plist::Value| -> Option<plist::Value> {
        match v {
            plist::Value::Uid(u) => objs.get(u.get() as usize).cloned(),
            other => Some(other.clone()),
        }
    };
    let root = d
        .get("$top")
        .and_then(|t| t.as_dictionary())
        .and_then(|t| t.get("root"))
        .and_then(deref)
        .or_else(|| objs.get(1).cloned());
    let Some(root) = root.and_then(|x| x.into_dictionary()) else { return r };
    let int = |k: &str| {
        root.get(k).and_then(|x| x.as_signed_integer().or_else(|| x.as_unsigned_integer().map(|u| u as i64)))
    };
    r.size = int("Size").map(|s| s.max(0) as u64);
    r.modified = int("LastModified");
    r.class = int("ProtectionClass").unwrap_or(0) as u32;
    r.key = root.get("EncryptionKey").and_then(deref).and_then(|k| match k {
        plist::Value::Data(d) => Some(d),
        plist::Value::Dictionary(d) => d.get("NS.data").and_then(|x| x.as_data()).map(|x| x.to_vec()),
        _ => None,
    });
    r
}

// ───────────── where files go ─────────────

/// (app id, folder name). Matched against the app or app group of a domain.
const APPS: &[(&str, &str)] = &[
    ("net.whatsapp.WhatsAppSMB", "WhatsApp Business"),
    ("net.whatsapp.WhatsApp", "WhatsApp"),
    ("ph.telegra.Telegraph", "Telegram"),
    ("com.viber", "Viber"),
    ("com.facebook.Messenger", "Messenger"),
    ("com.burbn.instagram", "Instagram"),
    ("org.whispersystems.signal", "Signal"),
    ("jp.naver.line", "LINE"),
    ("com.tencent.xin", "WeChat"),
    ("com.toyopagroup.picaboo", "Snapchat"),
    ("com.apple.VoiceMemos", "Voice Memos"),
    ("com.apple.notes", "Notes"),
    ("com.apple.mobilenotes", "Notes"),
    ("com.apple.FileProvider.LocalStorage", "Files (On My iPhone)"),
    ("com.apple.DocumentsApp", "Files (On My iPhone)"),
];

fn app_folder(domain: &str) -> Option<String> {
    let id = domain
        .strip_prefix("AppDomainGroup-")
        .or_else(|| domain.strip_prefix("AppDomain-"))?;
    let id = id.strip_prefix("group.").unwrap_or(id);
    let id = id.strip_suffix(".shared").unwrap_or(id);
    let known = APPS.iter().find(|(a, _)| id == *a || id.starts_with(&format!("{a}.")));
    Some(match known {
        Some((_, name)) => name.to_string(),
        None => format!("Apps/{id}"),
    })
}

/// Where a backed-up file goes in the results, or None to leave it out.
/// Without "everything", system settings, caches and app internals stay out.
fn destination(domain: &str, rel: &str, everything: bool) -> Option<PathBuf> {
    let noisy = rel.starts_with("Library/Caches/")
        || rel.contains("/Caches/")
        || rel.starts_with("tmp/")
        || rel.starts_with("Library/SplashBoard/")
        || rel.starts_with("Library/Saved Application State/")
        || rel.starts_with("Library/WebKit/")
        || rel.starts_with("Library/Cookies/");
    if noisy && !everything {
        return None;
    }
    let path = |top: &str, rest: &str| Some(PathBuf::from(top).join(clean_rel(rest)));
    match domain {
        "CameraRollDomain" => {
            if let Some(r) = rel.strip_prefix("Media/DCIM/") {
                return path(CAMERA_ROLL, r);
            }
            if let Some(r) = rel.strip_prefix("Media/PhotoData/Mutations/") {
                return path(&format!("{CAMERA_ROLL}/Edited Versions"), r.trim_start_matches("DCIM/"));
            }
            if rel.starts_with("Media/PhotoData/Thumbnails/") && !everything {
                return None;
            }
            path(&format!("{CAMERA_ROLL}/Other"), rel.trim_start_matches("Media/"))
        }
        "MediaDomain" => {
            if let Some(r) = rel.strip_prefix("Library/SMS/Attachments/") {
                return path("Messages", r);
            }
            if let Some(r) = rel.strip_prefix("Media/Recordings/") {
                return path("Voice Memos", r);
            }
            path("Media", rel)
        }
        "HomeDomain" => {
            if let Some(r) = rel.strip_prefix("Library/SMS/Attachments/") {
                return path("Messages", r);
            }
            if let Some(r) = rel.strip_prefix("Library/Recordings/") {
                return path("Voice Memos", r);
            }
            if !everything && !rel.starts_with("Documents/") && !rel.starts_with("Media/") {
                return None;
            }
            path("Home", rel)
        }
        d if d.starts_with("AppDomain-") || d.starts_with("AppDomainGroup-") => {
            let top = app_folder(d)?;
            // Voice Memos keeps recordings in "Recordings/", WhatsApp media in "Message/Media/".
            let rel = rel.strip_prefix("Recordings/").or_else(|| rel.strip_prefix("Message/")).unwrap_or(rel);
            path(&top, rel)
        }
        _ if everything => path(&format!("System/{domain}"), rel),
        _ => None,
    }
}

/// What the Photos library says about one photo or video.
#[derive(Default, Clone)]
struct PhotoInfo {
    trashed: bool,
    hidden: bool,
    /// The name it had when it was imported or saved (photos saved from apps
    /// get a new name in DCIM).
    original: Option<String>,
}

fn columns(conn: &rusqlite::Connection, table: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(mut st) = conn.prepare(&format!("PRAGMA table_info({table})")) {
        if let Ok(rows) = st.query_map([], |r| r.get::<_, String>(1)) {
            set.extend(rows.flatten().map(|c| c.to_ascii_uppercase()));
        }
    }
    set
}

/// Every photo and video in the Photos library (Photos.sqlite), by its path in
/// the backup ("Media/DCIM/100APPLE/IMG_0001.HEIC"). Columns differ between iOS
/// versions, so only the ones that exist are asked for.
fn photo_library(db: &Path) -> HashMap<String, PhotoInfo> {
    let mut map = HashMap::new();
    let Ok(conn) = rusqlite::Connection::open(db) else { return map };
    // ZASSET since iOS 14, ZGENERICASSET before.
    for table in ["ZASSET", "ZGENERICASSET"] {
        let cols = columns(&conn, table);
        if !cols.contains("ZDIRECTORY") || !cols.contains("ZFILENAME") {
            continue;
        }
        let col = |c: &str| if cols.contains(c) { format!("a.{c}") } else { "0".to_string() };
        let extra = columns(&conn, "ZADDITIONALASSETATTRIBUTES");
        let (orig, join) = if cols.contains("ZADDITIONALATTRIBUTES") && extra.contains("ZORIGINALFILENAME") {
            ("aa.ZORIGINALFILENAME", " LEFT JOIN ZADDITIONALASSETATTRIBUTES aa ON aa.Z_PK = a.ZADDITIONALATTRIBUTES")
        } else {
            ("NULL", "")
        };
        let sql = format!(
            "SELECT a.ZDIRECTORY, a.ZFILENAME, {}, {}, {orig} FROM {table} a{join}",
            col("ZTRASHEDSTATE"),
            col("ZHIDDEN")
        );
        let Ok(mut st) = conn.prepare(&sql) else { continue };
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                r.get::<_, Option<String>>(4)?,
            ))
        });
        if let Ok(rows) = rows {
            for (dir, file, trashed, hidden, original) in rows.flatten() {
                let (Some(dir), Some(file)) = (dir, file) else { continue };
                map.insert(
                    format!("Media/{dir}/{file}"),
                    PhotoInfo { trashed: trashed == 1, hidden: hidden == 1, original: original.filter(|o| !o.trim().is_empty()) },
                );
            }
        }
        if !map.is_empty() {
            break;
        }
    }
    map
}

/// Messages → Recently Deleted (iOS 16 and later): the deleted messages as
/// readable lines, and the backup paths of their attachments.
fn deleted_messages(db: &Path) -> (Vec<String>, HashSet<String>) {
    let mut lines = Vec::new();
    let mut attachments = HashSet::new();
    let Ok(conn) = rusqlite::Connection::open(db) else { return (lines, attachments) };
    if columns(&conn, "chat_recoverable_message_join").is_empty() {
        return (lines, attachments);
    }
    let sql = "SELECT m.text, m.attributedBody, m.date, m.is_from_me, h.id \
FROM chat_recoverable_message_join j JOIN message m ON m.ROWID = j.message_id \
LEFT JOIN handle h ON h.ROWID = m.handle_id ORDER BY m.date";
    if let Ok(mut st) = conn.prepare(sql) {
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<Vec<u8>>>(1)?,
                r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                r.get::<_, Option<String>>(4)?,
            ))
        });
        if let Ok(rows) = rows {
            for (text, body, date, from_me, who) in rows.flatten() {
                let text = text
                    .filter(|t| !t.trim().is_empty())
                    .or_else(|| body.as_deref().and_then(typedstream_text))
                    .unwrap_or_else(|| "(photo, video or other attachment)".into());
                let who = if from_me == 1 { "Me".to_string() } else { who.unwrap_or_else(|| "Unknown".into()) };
                lines.push(format!("{} · {who}: {}", apple_time(date), text.replace('\u{fffc}', "").trim()));
            }
        }
    }
    let sql = "SELECT a.filename FROM chat_recoverable_message_join j \
JOIN message_attachment_join maj ON maj.message_id = j.message_id \
JOIN attachment a ON a.ROWID = maj.attachment_id";
    if let Ok(mut st) = conn.prepare(sql) {
        if let Ok(rows) = st.query_map([], |r| r.get::<_, Option<String>>(0)) {
            for f in rows.flatten().flatten() {
                let rel = f.trim_start_matches("~/").trim_start_matches("/var/mobile/");
                attachments.insert(rel.to_string());
            }
        }
    }
    (lines, attachments)
}

/// Message dates count from 2001, in nanoseconds since iOS 11 (seconds before).
fn apple_time(t: i64) -> String {
    let secs = if t.abs() > 100_000_000_000 { t / 1_000_000_000 } else { t };
    chrono::DateTime::from_timestamp(secs + 978_307_200, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Unknown date".into())
}

/// The text inside a message's attributedBody (an Apple "typedstream"): the
/// string that follows the NSString class name.
fn typedstream_text(b: &[u8]) -> Option<String> {
    let at = b.windows(8).position(|w| w == b"NSString")? + 8;
    let rest = b.get(at..)?;
    let plus = rest.iter().take(16).position(|&x| x == b'+')? + 1;
    let rest = rest.get(plus..)?;
    let (len, skip) = match *rest.first()? {
        0x81 => (u16::from_le_bytes([*rest.get(1)?, *rest.get(2)?]) as usize, 3),
        0x82 => (u32::from_le_bytes([*rest.get(1)?, *rest.get(2)?, *rest.get(3)?, *rest.get(4)?]) as usize, 5),
        n => (n as usize, 1),
    };
    let text = String::from_utf8_lossy(rest.get(skip..skip + len)?).to_string();
    (!text.trim().is_empty()).then_some(text)
}

/// Voice memo titles (CloudRecordings.db), by recording file name.
fn voice_memo_titles(db: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(conn) = rusqlite::Connection::open(db) else { return map };
    for (table, label) in [("ZCLOUDRECORDING", "ZCUSTOMLABELFORSORTING"), ("ZCLOUDRECORDING", "ZCUSTOMLABEL"), ("ZRECORDING", "ZCUSTOMLABEL")] {
        let cols = columns(&conn, table);
        if !cols.contains("ZPATH") || !cols.contains(label) {
            continue;
        }
        let Ok(mut st) = conn.prepare(&format!("SELECT ZPATH, {label} FROM {table}")) else { continue };
        if let Ok(rows) = st.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?))) {
            for (path, title) in rows.flatten() {
                if let (Some(p), Some(t)) = (path, title.filter(|t| !t.trim().is_empty())) {
                    let name = p.rsplit('/').next().unwrap_or(&p).to_string();
                    map.insert(name, t.trim().to_string());
                }
            }
        }
        if !map.is_empty() {
            break;
        }
    }
    map
}

// ───────────── reading a backup ─────────────

struct Backup {
    dir: PathBuf,
    keybag: Option<Keybag>,
}

impl Backup {
    /// Where a file is stored in the backup. File ids are 40 hex digits; anything
    /// else (a damaged or doctored Manifest.db) could point outside the backup.
    fn source(&self, file_id: &str) -> Option<PathBuf> {
        (file_id.len() == 40 && file_id.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| self.dir.join(&file_id[..2]).join(file_id))
    }

    /// Copies (or decrypts) one file. Never touches the backup itself.
    fn extract(&self, file_id: &str, rec: &Record, dst: &Path) -> Result<u64, &'static str> {
        let src = self.source(file_id).ok_or("missing")?;
        if !src.is_file() {
            return Err("missing");
        }
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p).map_err(|_| "write")?;
        }
        let n = match &self.keybag {
            None => fs::copy(&src, dst).map_err(|_| "write")?,
            Some(kb) => {
                // An empty file has no key: there is nothing to decrypt.
                if rec.key.is_none() && rec.size == Some(0) {
                    return Err("empty");
                }
                let wrapped = rec.key.as_deref().filter(|k| k.len() >= 4).ok_or("locked")?;
                let key = kb.unwrap(rec.class, &wrapped[4..]).ok_or("locked")?;
                decrypt_file(&src, dst, &key).map_err(|_| "write")?
            }
        };
        if let Some(t) = rec.modified.filter(|t| *t > 0) {
            let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(t as u64);
            if let Ok(f) = fs::OpenOptions::new().write(true).open(dst) {
                let _ = f.set_modified(when);
            }
        }
        Ok(n)
    }
}

fn open_manifest(dir: &Path, password: Option<&str>, ctx: &mut Ctx) -> Result<(Backup, PathBuf), String> {
    let manifest = plist_dict(&dir.join("Manifest.plist")).ok_or("This backup is damaged (Manifest.plist can't be read).")?;
    let encrypted = manifest.get("IsEncrypted").and_then(|v| v.as_boolean()).unwrap_or(false);
    let db = ctx.work.join("Manifest.db");
    if !encrypted {
        fs::copy(dir.join("Manifest.db"), &db).map_err(|e| format!("The backup can't be read ({e})."))?;
        return Ok((Backup { dir: dir.to_path_buf(), keybag: None }, db));
    }
    let password = password.filter(|p| !p.is_empty()).ok_or(
        "This backup is encrypted. Enter the password you chose when you turned on “Encrypt local backup”.",
    )?;
    let bag = manifest.get("BackupKeyBag").and_then(|v| v.as_data()).ok_or("The backup's keys are missing.")?;
    let mut kb = Keybag::parse(bag);
    ctx.status("Unlocking the backup with its password. This takes a few seconds…");
    kb.unlock(password).map_err(|e| {
        if e == "wrong password" {
            "That password didn't unlock the backup. It's the one you chose when you turned on “Encrypt local backup” (not the iPhone's passcode).".to_string()
        } else {
            e
        }
    })?;
    let mk = manifest.get("ManifestKey").and_then(|v| v.as_data()).ok_or("The backup's keys are missing.")?;
    if mk.len() < 8 {
        return Err("The backup's keys are damaged.".into());
    }
    let class = u32::from_le_bytes([mk[0], mk[1], mk[2], mk[3]]);
    let key = kb.unwrap(class, &mk[4..]).ok_or("The backup's keys are damaged.")?;
    decrypt_file(&dir.join("Manifest.db"), &db, &key).map_err(|e| format!("The backup can't be read ({e})."))?;
    Ok((Backup { dir: dir.to_path_buf(), keybag: Some(kb) }, db))
}

struct Row {
    file_id: String,
    domain: String,
    rel: String,
    blob: Vec<u8>,
}

fn ext_lower(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

pub fn run(dir: &Path, password: Option<&str>, ctx: &mut Ctx) -> Result<(), String> {
    ctx.status("Opening the backup…");
    let (backup, db_path) = open_manifest(dir, password, ctx)?;
    let rows: Vec<Row> = {
        let conn = rusqlite::Connection::open(&db_path).map_err(|e| format!("The backup's file list can't be read ({e})."))?;
        let mut st = conn
            .prepare("SELECT fileID, domain, relativePath, file FROM Files WHERE flags = 1")
            .map_err(|e| format!("The backup's file list can't be read ({e})."))?;
        let it = st
            .query_map([], |r| {
                Ok(Row {
                    file_id: r.get(0)?,
                    domain: r.get(1)?,
                    rel: r.get(2)?,
                    blob: r.get::<_, Option<Vec<u8>>>(3)?.unwrap_or_default(),
                })
            })
            .map_err(|e| format!("The backup's file list can't be read ({e})."))?;
        it.flatten().collect()
    };
    let _ = fs::remove_file(&db_path);

    let by_path: HashMap<(&str, &str), &Row> = rows.iter().map(|r| ((r.domain.as_str(), r.rel.as_str()), r)).collect();
    // A database is read from a private copy, with its "-wal" and "-shm"
    // companions: recent changes (like a fresh deletion) are often only there.
    let work = ctx.work.clone();
    let mut db_n = 0;
    let mut copy_db = |row: &Row| -> Option<PathBuf> {
        db_n += 1;
        let dir = work.join(format!("db{db_n}"));
        let name = row.rel.rsplit('/').next().unwrap_or("db").to_string();
        let db = dir.join(&name);
        backup.extract(&row.file_id, &record(&row.blob), &db).ok()?;
        for suffix in ["-wal", "-shm"] {
            let rel = format!("{}{suffix}", row.rel);
            if let Some(r) = by_path.get(&(row.domain.as_str(), rel.as_str())) {
                let _ = backup.extract(&r.file_id, &record(&r.blob), &dir.join(format!("{name}{suffix}")));
            }
        }
        Some(db)
    };

    ctx.status("Reading the Photos library, Messages and Voice Memos…");
    let library = by_path
        .get(&("CameraRollDomain", "Media/PhotoData/Photos.sqlite"))
        .and_then(|r| copy_db(r))
        .map(|db| photo_library(&db))
        .unwrap_or_default();
    let sms = rows
        .iter()
        .find(|r| r.domain == "HomeDomain" && r.rel == "Library/SMS/sms.db")
        .and_then(&mut copy_db)
        .map(|db| deleted_messages(&db))
        .unwrap_or_default();
    let titles = rows
        .iter()
        .find(|r| r.rel.ends_with("Recordings/CloudRecordings.db") || r.rel.ends_with("Recordings/Recordings.db"))
        .and_then(&mut copy_db)
        .map(|db| voice_memo_titles(&db))
        .unwrap_or_default();
    for i in 1..=db_n {
        let _ = fs::remove_dir_all(work.join(format!("db{i}")));
    }
    let (deleted_lines, deleted_attachments) = sms;

    // Deleted messages, as a text file anyone can open.
    let mut deleted_texts = 0;
    if !deleted_lines.is_empty() && ctx.wants("Deleted messages.txt") {
        let file = ctx.out.join("Messages").join("Recently Deleted").join("Deleted messages.txt");
        let mut text = String::from(
            "Messages that were in Messages → Recently Deleted when the backup was made.\n\
Dates are in this computer's time zone.\n\n",
        );
        for l in &deleted_lines {
            text.push_str(l);
            text.push('\n');
        }
        if fs::create_dir_all(file.parent().unwrap_or(&ctx.out)).is_ok() && fs::write(&file, text).is_ok() {
            deleted_texts = deleted_lines.len();
            ctx.found(&file, 0);
        }
    }

    let everything = ctx.everything();
    let mut planned: Vec<(&Row, PathBuf)> = Vec::new();
    for r in &rows {
        let name = r.rel.rsplit('/').next().unwrap_or(&r.rel);
        let photo = (r.domain == "CameraRollDomain").then(|| library.get(&r.rel)).flatten();
        // A saved or imported photo gets its old name back, if the type matches.
        let shown = photo
            .and_then(|p| p.original.as_deref())
            .filter(|o| ext_lower(o) == ext_lower(name))
            .unwrap_or(name);
        let shown = match titles.get(name) {
            Some(t) if r.rel.contains("Recordings/") => format!("{t}.{}", ext_lower(name)),
            _ => shown.to_string(),
        };
        if !ctx.wants(&shown) {
            continue;
        }
        let dest = if photo.is_some_and(|p| p.trashed) {
            PathBuf::from(CAMERA_ROLL).join("Recently Deleted").join(clean_part(&shown))
        } else if photo.is_some_and(|p| p.hidden) {
            PathBuf::from(CAMERA_ROLL).join("Hidden").join(clean_part(&shown))
        } else if deleted_attachments.contains(&r.rel) {
            PathBuf::from("Messages").join("Recently Deleted").join(clean_part(&shown))
        } else {
            match destination(&r.domain, &r.rel, everything) {
                Some(d) => d.with_file_name(clean_part(&shown)),
                None => continue,
            }
        };
        planned.push((r, dest));
    }

    // Photos whose original is only in iCloud ("Optimize iPhone Storage"): the
    // backup may still hold a preview of them, which is better than nothing.
    let mut cloud_only = 0u64;
    let mut previews = 0u64;
    if ctx.wants("preview.jpg") {
        for (path, info) in &library {
            if by_path.contains_key(&("CameraRollDomain", path.as_str())) {
                continue;
            }
            cloud_only += 1;
            let rest = path.trim_start_matches("Media/");
            let thumb = format!("Media/PhotoData/Thumbnails/V2/{rest}/5005.JPG");
            if let Some(r) = by_path.get(&("CameraRollDomain", thumb.as_str())) {
                let stem = Path::new(rest).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let folder = if info.trashed { "Previews of Recently Deleted (Original in iCloud)" } else { "Previews Only (Original in iCloud)" };
                planned.push((r, PathBuf::from(CAMERA_ROLL).join(folder).join(clean_part(&format!("{stem}.jpg")))));
                previews += 1;
            }
        }
    }
    let trashed: HashSet<&str> = library.iter().filter(|(_, i)| i.trashed).map(|(p, _)| p.as_str()).collect();
    let total = planned.len() as u64;
    let camera_roll = rows.iter().filter(|r| r.domain == "CameraRollDomain" && r.rel.starts_with("Media/DCIM/")).count();
    let (mut done, mut bytes, mut locked, mut missing, mut write_err, mut recently_deleted) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    ctx.progress(0, total, 0);
    for (row, dest) in planned {
        if ctx.stopped() {
            break;
        }
        let rec = record(&row.blob);
        let dst = super::organize::unique(ctx.out.join(&dest));
        match backup.extract(&row.file_id, &rec, &dst) {
            Ok(n) => {
                let dst = super::devices::fix_name(&dst);
                bytes += n;
                if row.domain == "CameraRollDomain" && trashed.contains(row.rel.as_str()) {
                    recently_deleted += 1;
                }
                ctx.found(&dst, n);
            }
            Err("locked") => locked += 1,
            Err("empty") => {}
            Err("missing") => missing += 1,
            Err(_) => {
                write_err += 1;
                let _ = fs::remove_file(&dst);
                if write_err >= 20 && write_err > done / 2 {
                    return Err("Files can't be written to the save folder. Is it full?".into());
                }
            }
        }
        done += 1;
        ctx.progress(done, total, bytes);
    }

    ctx.notes.push(
        "Apple doesn't let any app read an iPhone's storage directly, so RecovMe read this backup instead. \
Everything that was on the device when the backup was made is here, even if it was deleted later."
            .into(),
    );
    if recently_deleted > 0 {
        ctx.notes.push(format!(
            "{recently_deleted} photo{} {} in Recently Deleted when the backup was made. {} in Camera Roll → Recently Deleted.",
            if recently_deleted == 1 { "" } else { "s" },
            if recently_deleted == 1 { "was" } else { "were" },
            if recently_deleted == 1 { "It is" } else { "They are" },
        ));
    }
    if deleted_texts > 0 {
        ctx.notes.push(format!(
            "{deleted_texts} message{} {} in Messages → Recently Deleted when the backup was made. They are in Messages → Recently Deleted → “Deleted messages.txt”, with any photos they had.",
            if deleted_texts == 1 { "" } else { "s" },
            if deleted_texts == 1 { "was" } else { "were" },
        ));
    }
    if cloud_only > 0 {
        ctx.notes.push(format!(
            "{cloud_only} item{} in the Photos library {} only in iCloud, not in this backup (“Optimize iPhone Storage” was on). {}Get the originals from icloud.com → Photos.",
            if cloud_only == 1 { "" } else { "s" },
            if cloud_only == 1 { "is" } else { "are" },
            if previews > 0 {
                format!("For {previews} of them the backup had a small preview, which is in Camera Roll. ")
            } else {
                String::new()
            }
        ));
    }
    if camera_roll == 0 {
        ctx.notes.push(
            "This backup has no Camera Roll photos. That happens when iCloud Photos is on: deleted photos are then in icloud.com → Photos → Recently Deleted for 30 days."
                .into(),
        );
    }
    if locked > 0 {
        ctx.notes.push(format!(
            "{locked} file{} can only be opened on the device itself (Apple locks some data to it), so {} left out.",
            if locked == 1 { "" } else { "s" },
            if locked == 1 { "it was" } else { "they were" }
        ));
    }
    if missing > 0 {
        ctx.notes.push(format!(
            "{missing} file{} listed in the backup {} missing from its folder: the backup may be incomplete.",
            if missing == 1 { "" } else { "s" },
            if missing == 1 { "is" } else { "are" }
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations() {
        let d = |dom: &str, rel: &str| destination(dom, rel, false).map(|p| p.to_string_lossy().replace('\\', "/"));
        assert_eq!(d("CameraRollDomain", "Media/DCIM/100APPLE/IMG_0001.HEIC").as_deref(), Some("Camera Roll/100APPLE/IMG_0001.HEIC"));
        assert_eq!(d("MediaDomain", "Library/SMS/Attachments/ab/01/X/IMG.jpg").as_deref(), Some("Messages/ab/01/X/IMG.jpg"));
        assert_eq!(
            d("AppDomainGroup-group.net.whatsapp.WhatsApp.shared", "Message/Media/123@s.whatsapp.net/a/b/x.jpg").as_deref(),
            Some("WhatsApp/Media/123@s.whatsapp.net/a/b/x.jpg")
        );
        assert_eq!(d("AppDomain-com.example.app", "Documents/a.pdf").as_deref(), Some("Apps/com.example.app/Documents/a.pdf"));
        assert_eq!(d("AppDomain-com.example.app", "Library/Caches/a.jpg"), None);
        assert_eq!(d("KeychainDomain", "keychain-backup.plist"), None);
    }

    #[test]
    fn message_text() {
        let body = b"\x04\x0bstreamtyped\x81\xe8\x03\x84\x01@\x84\x84\x84\x12NSAttributedString\x00\x84\x84\x08NSObject\x00\x85\x92\x84\x84\x84\x08NSString\x01\x94\x84\x01+\x0cSee you soon\x86";
        assert_eq!(typedstream_text(body).as_deref(), Some("See you soon"));
        assert_eq!(typedstream_text(b"nothing here"), None);
        assert!(apple_time(700_000_000_000_000_000).starts_with("2023-03-0"));
    }
}
