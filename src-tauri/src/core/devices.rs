// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
//! Phones and tablets. Their storage can't be searched block by block like a
//! memory card: iPhones never allow it, and Android phones encrypt their storage
//! and wipe deleted space. So instead RecovMe copies the places where deleted
//! and forgotten files really survive:
//!   Android (USB debugging, adb) – trash folders, thumbnail caches, chat app media
//!   iPhone / iPad – the backups Finder, iTunes or Apple Devices made on this
//!                   computer (encrypted ones too), including Recently Deleted photos
//! Copied files land in the same work folder Quick Scan uses, then the normal
//! organize step tidies them, and each source folder becomes a row in the results.

use super::android;
use super::categories::{category_of, previewable};
use super::engines::Engines;
use super::iosbackup;
use super::job::{new_session, remove_if_empty, Emit, Job};
use super::organize::{move_file, organize, unique, write_report, OrgOptions, ORIGINAL_NAMES};
use super::types::{CategoryCount, Drive, FoundFile, ScanEvent, ScanOptions, Summary};
use super::util::human_size;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub fn is_device(device: &str) -> bool {
    device.starts_with(android::PREFIX) || device.starts_with(iosbackup::PREFIX)
}

/// Connected Android phones and the iPhone / iPad backups on this computer.
pub fn list(engines: &Engines) -> Vec<Drive> {
    let mut v = Vec::new();
    if let Some(adb) = &engines.adb {
        v.extend(android::list(adb));
    }
    v.extend(iosbackup::list());
    v
}

/// A file picked with "Open…": an iPhone backup if it sits in one.
pub fn picked(path: &Path) -> Option<Drive> {
    iosbackup::backup_dir_of(path).and_then(|d| iosbackup::drive_for(&d).ok())
}

/// What a copier can use while it runs.
pub struct Ctx {
    /// Where copied files go (keeps their names and folders).
    pub out: PathBuf,
    /// The session's private work folder.
    pub work: PathBuf,
    pub categories: Vec<String>,
    /// Shown to the user on the results screen.
    pub notes: Vec<String>,
    stop: Arc<AtomicBool>,
    emit: Emit,
    label: String,
    files: u64,
    last_progress: Instant,
    previews: Vec<FoundFile>,
    last_previews: Instant,
}

impl Ctx {
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Only the kinds of file the user asked for. "Everything" also keeps app
    /// databases and settings; the others leave them out.
    pub fn wants(&self, name: &str) -> bool {
        let ext = ext_of_name(name);
        if self.everything() {
            return true;
        }
        if matches!(ext.as_str(), "db" | "sqlite" | "sqlite3" | "sqlitedb" | "plist") {
            return false;
        }
        // No extension: copy it and look inside (see `fix_name`).
        ext.is_empty() || self.categories.iter().any(|c| c == category_of(&ext))
    }

    pub fn everything(&self) -> bool {
        self.categories.is_empty() || self.categories.iter().any(|c| c == "everything")
    }

    /// Something is happening that has no percentage yet.
    pub fn status(&mut self, detail: &str) {
        (self.emit)(ScanEvent::Progress {
            phase: "copy".into(),
            label: self.label.clone(),
            percent: None,
            files_found: self.files,
            eta_secs: None,
            pass: None,
            detail: detail.into(),
        });
        self.last_progress = Instant::now();
    }

    pub fn progress(&mut self, done: u64, total: u64, bytes: u64) {
        if self.last_progress.elapsed() < Duration::from_millis(400) && done < total {
            return;
        }
        self.last_progress = Instant::now();
        (self.emit)(ScanEvent::Progress {
            phase: "copy".into(),
            label: self.label.clone(),
            percent: if total > 0 { Some((done as f32 / total as f32 * 100.0).min(100.0)) } else { None },
            files_found: self.files,
            eta_secs: None,
            pass: None,
            detail: if self.stopped() {
                "Stopping…".into()
            } else {
                format!("{} of {} files checked · {} copied", done, total, human_size(bytes))
            },
        });
    }

    /// A file was copied: count it, and show it in the live grid if it's a photo.
    pub fn found(&mut self, path: &Path, size: u64) {
        self.files += 1;
        let ext = ext_of_name(&path.to_string_lossy());
        if previewable(&ext) && size >= 20 * 1024 {
            self.previews.push(FoundFile {
                path: path.to_string_lossy().to_string(),
                name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                category: "photos".into(),
                size,
            });
        }
        if self.previews.len() >= 48 || (!self.previews.is_empty() && self.last_previews.elapsed() > Duration::from_millis(1500)) {
            self.flush_previews();
        }
    }

    fn flush_previews(&mut self) {
        if !self.previews.is_empty() {
            let mut files = std::mem::take(&mut self.previews);
            files.truncate(48);
            (self.emit)(ScanEvent::Files { files });
        }
        self.last_previews = Instant::now();
    }
}

pub fn start(opts: ScanOptions, engines: Engines, emit: Emit) -> Result<Job, String> {
    enum Source {
        Android { adb: PathBuf, serial: String },
        Backup { dir: PathBuf },
    }
    let source = if let Some(serial) = opts.device.strip_prefix(android::PREFIX) {
        let adb = engines
            .adb
            .clone()
            .ok_or("The Android tools are missing from this copy of the app. Please reinstall it.")?;
        Source::Android { adb, serial: serial.to_string() }
    } else {
        let dir = PathBuf::from(opts.device.trim_start_matches(iosbackup::PREFIX));
        if !dir.join("Manifest.db").is_file() && !dir.join("Manifest.plist").is_file() {
            return Err("That backup can't be found any more. Press refresh and pick it again.".into());
        }
        Source::Backup { dir }
    };

    let session = new_session(&opts)?;
    let work = session.join(".recovme");
    let out = work.join("quick");
    fs::create_dir_all(&out).map_err(|e| format!("Can't write to the save folder: {e}"))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let job = Job::new(session.clone(), work.join("stop"), stop_flag.clone(), finished.clone());
    let label = opts.device_label.clone();

    std::thread::spawn(move || {
        let t0 = Instant::now();
        let mut ctx = Ctx {
            out,
            work: work.clone(),
            categories: opts.categories.clone(),
            notes: Vec::new(),
            stop: stop_flag.clone(),
            emit: emit.clone(),
            label: match &source {
                Source::Android { .. } => format!("Copying from {label}"),
                Source::Backup { .. } => format!("Reading the backup of {label}"),
            },
            files: 0,
            last_progress: Instant::now(),
            previews: Vec::new(),
            last_previews: Instant::now(),
        };
        let result = match &source {
            Source::Android { adb, serial } => android::run(adb, serial, &opts.places, &mut ctx),
            Source::Backup { dir } => iosbackup::run(dir, opts.password.as_deref(), &mut ctx),
        };
        ctx.flush_previews();
        let stopped = stop_flag.load(Ordering::SeqCst);
        if let Err(msg) = result {
            if ctx.files == 0 {
                let _ = fs::remove_dir_all(&work);
                remove_if_empty(&session);
                finished.store(true, Ordering::SeqCst);
                emit(ScanEvent::Error { message: msg });
                return;
            }
            ctx.notes.push(format!("The copy stopped early: {msg} The files below were copied before that."));
        }

        let org = OrgOptions {
            skip_small: false,
            dedupe: false,
            rename_photos: false,
            categories: opts.categories.clone(),
            existing_roots: Vec::new(),
        };
        let em = emit.clone();
        let progress = move |done: u64, total: u64| {
            em(ScanEvent::Progress {
                phase: "organize".into(),
                label: "Organizing your files".into(),
                percent: if total > 0 { Some(done as f32 / total as f32 * 100.0) } else { None },
                files_found: total,
                eta_secs: None,
                pass: None,
                detail: "Keeping the kinds of file you chose, with their names and dates".into(),
            });
        };
        progress(0, ctx.files);
        let mut summary = organize(&session, &work, &org, &progress);
        regroup(&session, &mut summary);
        summary.stopped = stopped;
        summary.elapsed_secs = t0.elapsed().as_secs();
        summary.notes = std::mem::take(&mut ctx.notes);
        if summary.total == 0 && !stopped {
            summary.notes.push(match &source {
                Source::Android { .. } => "Nothing deleted was found on the phone. If the phone has a memory card, put it in a card reader and scan it as a drive. Photos backed up to Google Photos stay in its Trash for 60 days (photos.google.com → Trash).".into(),
                Source::Backup { .. } => "This backup doesn't hold any files of the kinds you chose. Try again with “All File Types”.".into(),
            });
        }
        // Nothing of the engines' kind is left in the work folder.
        let _ = fs::remove_dir(&work);
        write_report(&session, &summary, &label);
        finished.store(true, Ordering::SeqCst);
        emit(ScanEvent::Done(summary));
    });
    Ok(job)
}

/// Organize puts everything under "Original names"; for a phone, each folder
/// in there ("Recently Deleted", "WhatsApp", "Camera Roll"…) is more useful as
/// its own result, so move them up a level and list them.
fn regroup(session: &Path, s: &mut Summary) {
    let names = session.join(ORIGINAL_NAMES);
    let mut rows = Vec::new();
    for e in fs::read_dir(&names).into_iter().flatten().flatten() {
        let from = e.path();
        let folder = e.file_name().to_string_lossy().to_string();
        let to = unique(session.join(&folder));
        if fs::rename(&from, &to).is_err() {
            continue;
        }
        let (mut count, mut bytes) = (0u64, 0u64);
        for f in walkdir::WalkDir::new(&to).into_iter().flatten().filter(|f| f.file_type().is_file()) {
            count += 1;
            bytes += f.metadata().map(|m| m.len()).unwrap_or(0);
        }
        if count == 0 {
            let _ = fs::remove_dir_all(&to);
            continue;
        }
        let key = folder_icon(&folder);
        rows.push(CategoryCount {
            key: key.into(),
            label: to.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(folder),
            count,
            bytes,
            path: to.to_string_lossy().to_string(),
        });
    }
    let _ = fs::remove_dir(&names);
    let rank = |r: &CategoryCount| match r.key.as_str() {
        "trash" => 0,
        "photos" => 1,
        _ => 2,
    };
    rows.sort_by(|a, b| rank(a).cmp(&rank(b)).then(b.count.cmp(&a.count)));
    let prefix = names.to_string_lossy().to_string();
    let top = session.to_string_lossy().to_string();
    for p in s.sample_photos.iter_mut() {
        if let Some(rest) = p.strip_prefix(&prefix) {
            *p = format!("{top}{rest}");
        }
    }
    s.total = rows.iter().map(|r| r.count).sum();
    s.original_names = 0;
    s.categories = rows;
}

/// Icon for a result folder (the app has matching drawings).
fn folder_icon(folder: &str) -> &'static str {
    match folder {
        android::TRASH_FOLDER => "trash",
        iosbackup::CAMERA_ROLL | android::THUMBS_FOLDER => "photos",
        android::STORAGE_FOLDER => "phone",
        "Voice Memos" => "audio",
        "Notes" | "Files (On My iPhone)" => "documents",
        "Messages" | "WhatsApp" | "WhatsApp Business" | "Telegram" | "Viber" | "Messenger" | "Signal"
        | "LINE" | "WeChat" | "Snapchat" | "Instagram" => "chat",
        _ => "other",
    }
}

fn ext_of_name(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// One path part, made safe for Windows and macOS and visible in Finder and
/// Explorer (phones hide folders like ".thumbnails" and ".Statuses").
pub fn clean_part(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| match c {
            '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let t = t.trim_start_matches('.').trim_end_matches(['.', ' ']).trim().to_string();
    if t.is_empty() {
        "_".into()
    } else {
        t
    }
}

/// A relative path from a phone, made safe part by part.
pub fn clean_rel(rel: &str) -> PathBuf {
    rel.split('/')
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .map(clean_part)
        .collect()
}

/// Android's trash renames "IMG_1.jpg" to ".trashed-1712345678-IMG_1.jpg"
/// (the number is when it will be erased). Unfinished photos and downloads are
/// ".pending-<number>-<name>".
pub fn untrash_name(name: &str) -> Option<&str> {
    let low = name.get(..9).map(|p| p.to_ascii_lowercase()).unwrap_or_default();
    let rest = if low == ".trashed-" || low == ".pending-" { &name[9..] } else { return None };
    let (stamp, original) = rest.split_once('-')?;
    (stamp.chars().all(|c| c.is_ascii_digit()) && !original.is_empty()).then_some(original)
}

/// Trash folders and caches often keep files without an extension. Look at the
/// first bytes to tell what it is, so it opens and sorts like any other file.
pub fn sniff_ext(p: &Path) -> Option<&'static str> {
    use std::io::Read;
    let mut b = [0u8; 16];
    let n = fs::File::open(p).ok()?.read(&mut b).ok()?;
    let b = &b[..n];
    let ftyp = |brands: &[&[u8]]| b.len() >= 12 && &b[4..8] == b"ftyp" && brands.iter().any(|x| &b[8..12] == *x);
    let ext = if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if b.starts_with(b"\x89PNG") {
        "png"
    } else if b.starts_with(b"GIF8") {
        "gif"
    } else if b.starts_with(b"RIFF") && b.len() >= 12 && &b[8..12] == b"WEBP" {
        "webp"
    } else if b.starts_with(b"RIFF") && b.len() >= 12 && &b[8..12] == b"WAVE" {
        "wav"
    } else if ftyp(&[b"heic", b"heix", b"mif1", b"msf1", b"hevc"]) {
        "heic"
    } else if ftyp(&[b"qt  "]) {
        "mov"
    } else if ftyp(&[b"M4A ", b"M4B "]) {
        "m4a"
    } else if ftyp(&[b"3gp4", b"3gp5", b"3gp6", b"3ge6", b"3gs7"]) {
        "3gp"
    } else if b.len() >= 8 && &b[4..8] == b"ftyp" {
        "mp4"
    } else if b.starts_with(b"%PDF") {
        "pdf"
    } else if b.starts_with(b"ID3") || (b.len() >= 2 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0) {
        "mp3"
    } else if b.starts_with(b"OggS") {
        "ogg"
    } else if b.starts_with(b"#!AMR") {
        "amr"
    } else if b.starts_with(b"PK\x03\x04") {
        "zip"
    } else {
        return None;
    };
    Some(ext)
}

/// Gives a copied file its real name back: removes Android's trash prefix and
/// adds the extension a trash folder left off. Returns where the file is now.
pub fn fix_name(p: &Path) -> PathBuf {
    let Some(name) = p.file_name().map(|n| n.to_string_lossy().to_string()) else { return p.to_path_buf() };
    let mut new = untrash_name(&name).map(clean_part).unwrap_or_else(|| name.clone());
    if ext_of_name(&new).is_empty() || category_of(&ext_of_name(&new)) == "other" {
        if let Some(ext) = sniff_ext(p) {
            new = format!("{}.{ext}", new.trim_end_matches('.'));
        }
    }
    if new == name {
        return p.to_path_buf();
    }
    let dest = unique(p.with_file_name(&new));
    match move_file(p, &dest) {
        Ok(()) => dest,
        Err(_) => p.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(untrash_name(".trashed-1712345678-IMG_1.jpg"), Some("IMG_1.jpg"));
        assert_eq!(untrash_name(".trashed-x-IMG.jpg"), None);
        assert_eq!(untrash_name("IMG.jpg"), None);
        assert_eq!(untrash_name(".pending-1712345678-VID_1.mp4"), Some("VID_1.mp4"));
        assert_eq!(untrash_name(".Trashed-1712345678-a.jpg"), Some("a.jpg"));
        assert_eq!(clean_part(".thumbnails"), "thumbnails");
        assert_eq!(clean_part("a:b?"), "a-b-");
        assert_eq!(clean_rel("../.Statuses/x.jpg"), PathBuf::from("Statuses").join("x.jpg"));
    }
}
