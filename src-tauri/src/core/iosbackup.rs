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

use super::devices::{clean_rel, Ctx};
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
/// videos don't have to fit in memory. Removes the padding and, when the real
/// size is known, cuts the file to it.
fn decrypt_file(src: &Path, dst: &Path, key: &Key, size: Option<u64>) -> std::io::Result<u64> {
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
    drop(w);
    if let Some(s) = size {
        if s < written {
            fs::OpenOptions::new().write(true).open(dst)?.set_len(s)?;
            written = s;
        }
    }
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

/// Paths (as in the backup) of the photos that were in Recently Deleted,
/// read from the Photos library database inside the backup.
fn trashed_photos(db: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    let Ok(conn) = rusqlite::Connection::open(db) else { return set };
    for table in ["ZASSET", "ZGENERICASSET"] {
        let sql = format!("SELECT ZDIRECTORY, ZFILENAME FROM {table} WHERE ZTRASHEDSTATE = 1");
        let Ok(mut st) = conn.prepare(&sql) else { continue };
        let rows = st.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)));
        if let Ok(rows) = rows {
            for (dir, file) in rows.flatten().filter_map(|(d, f)| Some((d?, f?))) {
                set.insert(format!("Media/{dir}/{file}"));
            }
        }
        if !set.is_empty() {
            break;
        }
    }
    set
}

// ───────────── reading a backup ─────────────

struct Backup {
    dir: PathBuf,
    keybag: Option<Keybag>,
}

impl Backup {
    fn source(&self, file_id: &str) -> PathBuf {
        self.dir.join(file_id.get(..2).unwrap_or("")).join(file_id)
    }

    /// Copies (or decrypts) one file. Never touches the backup itself.
    fn extract(&self, file_id: &str, rec: &Record, dst: &Path) -> Result<u64, &'static str> {
        let src = self.source(file_id);
        if !src.is_file() {
            return Err("missing");
        }
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p).map_err(|_| "write")?;
        }
        let n = match &self.keybag {
            None => fs::copy(&src, dst).map_err(|_| "write")?,
            Some(kb) => {
                let wrapped = rec.key.as_deref().filter(|k| k.len() >= 4).ok_or("locked")?;
                let key = kb.unwrap(rec.class, &wrapped[4..]).ok_or("locked")?;
                decrypt_file(&src, dst, &key, rec.size).map_err(|_| "write")?
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
    decrypt_file(&dir.join("Manifest.db"), &db, &key, None).map_err(|e| format!("The backup can't be read ({e})."))?;
    Ok((Backup { dir: dir.to_path_buf(), keybag: Some(kb) }, db))
}

struct Row {
    file_id: String,
    domain: String,
    rel: String,
    blob: Vec<u8>,
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

    // Which Camera Roll photos were in Recently Deleted.
    let mut trashed = HashSet::new();
    let lib = rows
        .iter()
        .find(|r| r.domain == "CameraRollDomain" && r.rel == "Media/PhotoData/Photos.sqlite");
    if let Some(lib) = lib {
        let tmp = ctx.work.join("photos");
        let db = tmp.join("Photos.sqlite");
        if backup.extract(&lib.file_id, &record(&lib.blob), &db).is_ok() {
            for suffix in ["-wal", "-shm"] {
                let rel = format!("Media/PhotoData/Photos.sqlite{suffix}");
                if let Some(r) = rows.iter().find(|r| r.domain == "CameraRollDomain" && r.rel == rel) {
                    let _ = backup.extract(&r.file_id, &record(&r.blob), &tmp.join(format!("Photos.sqlite{suffix}")));
                }
            }
            trashed = trashed_photos(&db);
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    let everything = ctx.everything();
    let planned: Vec<(&Row, PathBuf)> = rows
        .iter()
        .filter_map(|r| {
            let name = r.rel.rsplit('/').next().unwrap_or(&r.rel);
            if !ctx.wants(name) {
                return None;
            }
            let dest = if r.domain == "CameraRollDomain" && trashed.contains(&r.rel) {
                PathBuf::from(CAMERA_ROLL).join("Recently Deleted").join(super::devices::clean_part(name))
            } else {
                destination(&r.domain, &r.rel, everything)?
            };
            Some((r, dest))
        })
        .collect();
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
                if trashed.contains(&row.rel) {
                    recently_deleted += 1;
                }
                ctx.found(&dst, n);
            }
            Err("locked") => locked += 1,
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
}
