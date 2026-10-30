// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
//! Turns raw engine output into a tidy folder:
//!   Original names/…      Quick Scan results with their real names and folders
//!   Photos/2024-07/2024-07-12 14.33.02.jpg   (renamed by the date the photo was taken)
//!   Videos/MP4/…  Audio/MP3/…  Documents/PDF/…  Archives/ZIP/…  Other/…
//! Duplicates are removed (size first, then BLAKE3 only when sizes collide),
//! empty files are dropped, tiny embedded thumbnails are set aside, files whose
//! content the drive already wiped (all zeros) are set aside, and only the kinds
//! of file the user asked for are kept.

use super::categories::{category_of, folder_name, has_exif, is_structure, previewable, ORDER};
use super::progress::is_engine_report;
use super::types::{CategoryCount, Summary};
use chrono::{Datelike, NaiveDateTime};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub struct OrgOptions {
    pub skip_small: bool,
    pub dedupe: bool,
    pub rename_photos: bool,
    /// "photos", "videos", … or "everything".
    pub categories: Vec<String>,
    /// Where the scanned drive is mounted. When set, recovered files that are
    /// identical to a file still on the drive are left out (they were never lost).
    pub existing_roots: Vec<PathBuf>,
}

impl OrgOptions {
    fn wants(&self, cat: &str) -> bool {
        self.categories.is_empty()
            || self.categories.iter().any(|c| c == "everything" || c == cat)
    }
}

/// Image files smaller than this are almost always thumbnails embedded in other files.
const SMALL_IMAGE_BYTES: u64 = 25 * 1024;
pub const ORIGINAL_NAMES: &str = "Original names";
const SMALL_FOLDER: &str = "_Small images (probably thumbnails)";
pub const ERASED_FOLDER: &str = "_Empty files (erased by the drive)";

/// True when every byte is 0x00 (or every byte 0xFF): what an SSD returns for
/// space it has already wiped (TRIM). Small files are read fully, big ones sampled.
fn is_blank(p: &Path, size: u64) -> bool {
    use std::io::{Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(p) else { return false };
    const CHUNK: usize = 64 * 1024;
    let mut buf = vec![0u8; CHUNK];
    let mut fill: Option<u8> = None;
    let mut uniform = |data: &[u8]| -> bool {
        let Some(&first) = data.first() else { return true };
        if first != 0x00 && first != 0xFF {
            return false;
        }
        if *fill.get_or_insert(first) != first {
            return false;
        }
        data.iter().all(|&b| b == first)
    };
    if size <= 8 * 1024 * 1024 {
        loop {
            let n = match f.read(&mut buf) {
                Ok(0) => return true,
                Ok(n) => n,
                Err(_) => return false,
            };
            if !uniform(&buf[..n]) {
                return false;
            }
        }
    }
    for at in [0, size / 3, size * 2 / 3, size.saturating_sub(CHUNK as u64)] {
        if f.seek(SeekFrom::Start(at)).is_err() {
            return false;
        }
        let n = f.read(&mut buf).unwrap_or(0);
        if !uniform(&buf[..n]) {
            return false;
        }
    }
    true
}

type Hash = [u8; 32];

fn full_hash(p: &Path) -> Option<Hash> {
    let f = fs::File::open(p).ok()?;
    let mut h = blake3::Hasher::new();
    let mut r = BufReader::with_capacity(1 << 20, f);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = r.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(*h.finalize().as_bytes())
}

/// Two-stage duplicate finder: files only get hashed when another file has the same size.
#[derive(Default)]
struct Deduper {
    by_size: HashMap<u64, Vec<(PathBuf, Option<Hash>)>>,
}

impl Deduper {
    fn is_duplicate(&mut self, candidate: &Path, size: u64) -> (bool, Option<Hash>) {
        let Some(entries) = self.by_size.get_mut(&size) else { return (false, None) };
        let Some(ch) = full_hash(candidate) else { return (false, None) };
        for (p, h) in entries.iter_mut() {
            if h.is_none() {
                *h = full_hash(p);
            }
            if *h == Some(ch) {
                return (true, Some(ch));
            }
        }
        (false, Some(ch))
    }
    fn add(&mut self, path: PathBuf, size: u64, hash: Option<Hash>) {
        self.by_size.entry(size).or_default().push((path, hash));
    }
}

/// Sizes of the files still on the scanned drive (hashed later, only on a size match).
/// Capped so a huge drive can't stall the end of a scan.
fn index_existing(roots: &[PathBuf]) -> Deduper {
    const SKIP: [&str; 6] = [
        ".Spotlight-V100",
        ".fseventsd",
        ".DocumentRevisions-V100",
        ".TemporaryItems",
        "System Volume Information",
        ".recovme",
    ];
    let mut idx = Deduper::default();
    let started = std::time::Instant::now();
    let mut n = 0u32;
    for root in roots.iter().filter(|r| r.is_dir()) {
        let walk = walkdir::WalkDir::new(root)
            .same_file_system(true)
            .into_iter()
            .filter_entry(|e| !SKIP.iter().any(|s| e.file_name() == *s));
        for e in walk.filter_map(|e| e.ok()) {
            if !e.file_type().is_file() {
                continue;
            }
            if let Ok(m) = e.metadata() {
                if m.len() > 0 {
                    idx.add(e.into_path(), m.len(), None);
                }
            }
            n += 1;
            if n >= 400_000 || started.elapsed().as_secs() > 60 {
                return idx;
            }
        }
    }
    idx
}

fn ext_of(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// When a photo or video was taken, read from inside the file itself (the
/// drive's own dates are lost once a file is deleted or the drive formatted):
/// EXIF for photos, the movie header for MP4/MOV-style videos.
fn content_date(p: &Path, ext: &str) -> Option<NaiveDateTime> {
    if has_exif(ext) {
        photo_date(p)
    } else if matches!(ext, "mp4" | "mov" | "m4v" | "3gp" | "3g2" | "mj2") {
        video_date(p)
    } else {
        None
    }
}

fn plausible(dt: NaiveDateTime) -> Option<NaiveDateTime> {
    (1990..=2100).contains(&dt.year()).then_some(dt)
}

/// "2024:07:12 14:33:02" from EXIF (already local camera time).
fn photo_date(p: &Path) -> Option<NaiveDateTime> {
    let f = fs::File::open(p).ok()?;
    let mut r = BufReader::new(f);
    let exif = exif::Reader::new().read_from_container(&mut r).ok()?;
    let field = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTimeDigitized, exif::In::PRIMARY))
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;
    let raw = match &field.value {
        exif::Value::Ascii(v) => String::from_utf8_lossy(v.first()?).to_string(),
        _ => return None,
    };
    let raw = raw.get(0..19)?;
    plausible(NaiveDateTime::parse_from_str(raw, "%Y:%m:%d %H:%M:%S").ok()?)
}

/// Creation time from the MP4/MOV movie header (moov → mvhd), stored as
/// seconds since 1904 in UTC; returned in this computer's local time.
fn video_date(p: &Path) -> Option<NaiveDateTime> {
    use std::io::{Seek, SeekFrom};
    let mut f = fs::File::open(p).ok()?;
    let len = f.metadata().ok()?.len();
    let mut find = |start: u64, end: u64, name: &[u8; 4]| -> Option<(u64, u64)> {
        let mut pos = start;
        for _ in 0..4096 {
            if pos + 8 > end {
                return None;
            }
            f.seek(SeekFrom::Start(pos)).ok()?;
            let mut h = [0u8; 8];
            f.read_exact(&mut h).ok()?;
            let size32 = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
            let (hdr, size) = match size32 {
                1 => {
                    let mut b = [0u8; 8];
                    f.read_exact(&mut b).ok()?;
                    (16, u64::from_be_bytes(b))
                }
                0 => (8, end - pos),
                n => (8, n),
            };
            if size < hdr || pos + size > end {
                return None;
            }
            if &h[4..8] == name {
                return Some((pos + hdr, pos + size));
            }
            pos += size;
        }
        None
    };
    let moov = find(0, len, b"moov")?;
    let mvhd = find(moov.0, moov.1, b"mvhd")?;
    f.seek(SeekFrom::Start(mvhd.0)).ok()?;
    let mut b = [0u8; 12];
    f.read_exact(&mut b).ok()?;
    let secs_1904 = if b[0] == 1 {
        u64::from_be_bytes([b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11]])
    } else {
        u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as u64
    };
    let unix = secs_1904.checked_sub(2_082_844_800)? as i64;
    let utc = chrono::DateTime::from_timestamp(unix, 0)?;
    plausible(utc.with_timezone(&chrono::Local).naive_local())
}

fn month_folder(dt: NaiveDateTime) -> String {
    dt.format("%Y-%m").to_string()
}

fn date_name(dt: NaiveDateTime) -> String {
    dt.format("%Y-%m-%d %H.%M.%S").to_string()
}

/// Gives the recovered file its original "created" and "modified" dates, so
/// Finder, Explorer and photo apps sort it by when it was taken, not by today.
fn set_file_dates(p: &Path, dt: NaiveDateTime) {
    use chrono::TimeZone;
    let Some(local) = chrono::Local.from_local_datetime(&dt).earliest() else { return };
    let Ok(secs) = u64::try_from(local.timestamp()) else { return };
    let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    let times = fs::FileTimes::new().set_accessed(when).set_modified(when);
    #[cfg(target_os = "macos")]
    let times = {
        use std::os::macos::fs::FileTimesExt;
        times.set_created(when)
    };
    #[cfg(windows)]
    let times = {
        use std::os::windows::fs::FileTimesExt;
        times.set_created(when)
    };
    if let Ok(f) = fs::OpenOptions::new().write(true).open(p) {
        let _ = f.set_times(times);
    }
}

pub(crate) fn unique(dest: PathBuf) -> PathBuf {
    if !dest.exists() {
        return dest;
    }
    let stem = dest.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = dest.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    let parent = dest.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    for i in 2..100_000 {
        let c = parent.join(format!("{stem} ({i}){ext}"));
        if !c.exists() {
            return c;
        }
    }
    dest
}

pub(crate) fn move_file(src: &Path, dest: &Path) -> std::io::Result<()> {
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p)?;
    }
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(src, dest)?;
            fs::remove_file(src)
        }
    }
}

pub fn organize(
    session: &Path,
    work: &Path,
    opts: &OrgOptions,
    progress: &dyn Fn(u64, u64),
) -> Summary {
    let quick_dir = work.join("quick");
    let deep_dir = work.join("deep");

    let quick_files: Vec<PathBuf> = walkdir::WalkDir::new(&quick_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    let mut deep_files: Vec<PathBuf> = walkdir::WalkDir::new(&deep_dir)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && !is_engine_report(&e.file_name().to_string_lossy()))
        .map(|e| e.into_path())
        .collect();
    deep_files.sort();
    let total = (quick_files.len() + deep_files.len()) as u64;

    let mut dedupe = Deduper::default();
    let mut existing = index_existing(&opts.existing_roots);
    let mut counts: HashMap<&'static str, (u64, u64)> = HashMap::new();
    let mut summary = Summary {
        session_dir: session.to_string_lossy().to_string(),
        original_names_path: session.join(ORIGINAL_NAMES).to_string_lossy().to_string(),
        ..Default::default()
    };
    let mut photos_for_preview: Vec<(u64, PathBuf)> = Vec::new();
    let mut done = 0u64;

    // 1) Quick Scan results keep their real names and folder structure.
    let orig_root = session.join(ORIGINAL_NAMES);
    for src in quick_files {
        done += 1;
        if done % 200 == 0 {
            progress(done, total);
        }
        let size = fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
        if size == 0 || !opts.wants(category_of(&ext_of(&src))) {
            let _ = fs::remove_file(&src);
            continue;
        }
        let rel = src.strip_prefix(&quick_dir).unwrap_or(&src).to_path_buf();
        if is_blank(&src, size) {
            // Name survived but the drive already erased the content (typical on SSDs).
            if move_file(&src, &unique(session.join(ERASED_FOLDER).join(&rel))).is_ok() {
                summary.erased += 1;
            }
            continue;
        }
        let dest = unique(orig_root.join(rel));
        if move_file(&src, &dest).is_ok() {
            if let Some(dt) = content_date(&dest, &ext_of(&dest)) {
                set_file_dates(&dest, dt);
            }
            if opts.dedupe {
                // Hashed lazily, only if a Deep Scan file turns up with the same size.
                dedupe.add(dest.clone(), size, None);
            }
            summary.original_names += 1;
            summary.total_bytes += size;
            let ext = ext_of(&dest);
            if category_of(&ext) == "photos" && previewable(&ext) && size >= SMALL_IMAGE_BYTES {
                photos_for_preview.push((size, dest));
            }
        }
    }

    // 2) Deep Scan results get sorted, renamed and de-duplicated.
    for src in deep_files {
        done += 1;
        if done % 200 == 0 {
            progress(done, total);
        }
        let size = fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
        if size == 0 || is_structure(&ext_of(&src)) || !opts.wants(category_of(&ext_of(&src))) {
            let _ = fs::remove_file(&src);
            continue;
        }
        if !existing.by_size.is_empty() && existing.is_duplicate(&src, size).0 {
            let _ = fs::remove_file(&src);
            summary.still_on_drive += 1;
            continue;
        }
        let mut hash = None;
        if opts.dedupe {
            let (dup, h) = dedupe.is_duplicate(&src, size);
            if dup {
                let _ = fs::remove_file(&src);
                summary.duplicates_removed += 1;
                continue;
            }
            hash = h;
        }
        let ext = ext_of(&src);
        let cat = category_of(&ext);
        let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let cat_dir = session.join(folder_name(cat));
        let small = opts.skip_small
            && cat == "photos"
            && size < SMALL_IMAGE_BYTES
            && matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "ico");
        let dated = if matches!(cat, "photos" | "videos") { content_date(&src, &ext) } else { None };
        let dest = if small {
            summary.small_set_aside += 1;
            cat_dir.join(SMALL_FOLDER).join(&name)
        } else if cat == "photos" || (cat == "videos" && opts.rename_photos && dated.is_some()) {
            match dated.filter(|_| opts.rename_photos) {
                Some(dt) => cat_dir.join(month_folder(dt)).join(format!("{}.{ext}", date_name(dt))),
                None => cat_dir.join("No date").join(&name),
            }
        } else {
            let sub = if ext.is_empty() { "Unknown".to_string() } else { ext.to_ascii_uppercase() };
            cat_dir.join(sub).join(&name)
        };
        let dest = unique(dest);
        if move_file(&src, &dest).is_ok() {
            if let Some(dt) = dated {
                set_file_dates(&dest, dt);
            }
            if opts.dedupe {
                dedupe.add(dest.clone(), size, hash);
            }
            if !small {
                let e = counts.entry(cat).or_default();
                e.0 += 1;
                e.1 += size;
                if cat == "photos" && previewable(&ext) {
                    photos_for_preview.push((size, dest));
                }
            }
            summary.total_bytes += size;
        }
    }
    progress(total, total);

    for (key, label) in ORDER {
        if let Some((c, b)) = counts.get(key) {
            summary.categories.push(CategoryCount {
                key: key.to_string(),
                label: label.to_string(),
                count: *c,
                bytes: *b,
                path: session.join(label).to_string_lossy().to_string(),
            });
        }
    }
    summary.total = summary.original_names + counts.values().map(|v| v.0).sum::<u64>();

    // Biggest images first: those are the real photos, not icons.
    photos_for_preview.sort_by(|a, b| b.0.cmp(&a.0));
    summary.sample_photos = photos_for_preview
        .into_iter()
        .take(24)
        .map(|(_, p)| p.to_string_lossy().to_string())
        .collect();

    // Keep the engine logs, drop the now-empty raw folders.
    if let Some(r) = walkdir::WalkDir::new(&deep_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name() == "report.xml")
    {
        let _ = fs::rename(r.path(), work.join("photorec-report.xml"));
    }
    let _ = fs::remove_dir_all(&deep_dir);
    let _ = fs::remove_dir_all(&quick_dir);
    summary
}

pub fn write_report(session: &Path, s: &Summary, device_label: &str) {
    let mut t = String::new();
    t.push_str("RecovMe recovery report\n=======================\n\n");
    t.push_str(&format!("Drive: {device_label}\n"));
    t.push_str(&format!("Finished: {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M")));
    t.push_str(&format!("Time taken: {} min\n", s.elapsed_secs / 60));
    if s.stopped {
        t.push_str("Stopped early: yes (results below are what was found until then)\n");
    }
    t.push_str(&format!("\nFiles recovered: {}\n", s.total));
    if s.original_names > 0 || s.categories.is_empty() {
        t.push_str(&format!("  With original names: {}\n", s.original_names));
    }
    for c in &s.categories {
        t.push_str(&format!("  {}: {}\n", c.label, c.count));
    }
    t.push_str(&format!("Duplicates removed: {}\n", s.duplicates_removed));
    t.push_str(&format!("Small thumbnails set aside: {}\n", s.small_set_aside));
    if s.still_on_drive > 0 {
        t.push_str(&format!("Left out because they are still on the drive: {}\n", s.still_on_drive));
    }
    if s.erased > 0 {
        t.push_str(&format!("Empty files (already erased by the drive): {}\n", s.erased));
    }
    for n in &s.notes {
        t.push_str(&format!("\nNote: {n}\n"));
    }
    t.push_str("\nTip: recovered files can be damaged if parts of them were overwritten.\nOpen the important ones and copy them to a safe place (and a backup).\n");
    let _ = fs::write(session.join("Recovery report.txt"), t);
}
