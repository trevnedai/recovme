// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
//! Runs a scan: Quick Scan (tsk_recover, original names) then Deep Scan
//! (PhotoRec carving), with live progress, Stop, and tidy results.
//!
//! The engines need raw disk access:
//!   macOS   – one password prompt (osascript "with administrator privileges")
//!             runs a small shell runner as root in the background.
//!   Windows – the app itself runs as administrator (manifest), engines run directly.
//!   Linux   – runs directly as root, otherwise through pkexec.
//! Every runner writes the same status file, so one monitor loop drives the UI.

use super::categories::{photorec_fileopt, previewable, quick_supported};
use super::drives::check_output_not_on_source;
use super::engines::Engines;
use super::organize::{organize, write_report, OrgOptions};
use super::progress::{count_tree, parse_deep, read_status, tail, DeepTracker, Screen};
use super::types::{FoundFile, ScanEvent, ScanMode, ScanOptions};
use super::util::{human_size, safe_name};
#[allow(unused_imports)]
use super::util::{cmd, shq};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub type Emit = Arc<dyn Fn(ScanEvent) + Send + Sync>;

pub struct Job {
    pub session: PathBuf,
    stop_file: PathBuf,
    stop_flag: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}

impl Job {
    pub(crate) fn new(session: PathBuf, stop_file: PathBuf, stop_flag: Arc<AtomicBool>, finished: Arc<AtomicBool>) -> Self {
        Job { session, stop_file, stop_flag, finished }
    }
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        let _ = fs::write(&self.stop_file, b"stop");
    }
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }
}

struct Step {
    phase: &'static str,
    program: PathBuf,
    args: Vec<String>,
    out: PathBuf,
}

pub fn start(opts: ScanOptions, engines: Engines, emit: Emit) -> Result<Job, String> {
    // Phones and phone backups are copied from, not scanned block by block.
    if super::devices::is_device(&opts.device) {
        return super::devices::start(opts, engines, emit);
    }
    // ── Decide which engines run ──
    let fs_ok = quick_supported(opts.filesystem.as_deref())
        && !matches!(opts.filesystem.as_deref(), Some("apfs") | Some("hfs"));
    let mut notes: Vec<String> = Vec::new();
    let want_quick = matches!(opts.mode, ScanMode::Smart | ScanMode::Quick);
    let want_deep = matches!(opts.mode, ScanMode::Smart | ScanMode::Deep);
    let do_quick = want_quick && fs_ok && engines.tsk_recover.is_some();
    let do_deep = want_deep && engines.photorec.is_some();
    if opts.mode == ScanMode::Quick && !do_quick {
        return Err(if !fs_ok {
            "Quick Scan can't read deleted file names on this kind of drive. Use Deep Scan instead.".into()
        } else {
            "The Quick Scan engine is missing from this copy of the app.".into()
        });
    }
    if opts.mode == ScanMode::Deep && !do_deep {
        return Err("The Deep Scan engine (PhotoRec) is missing from this copy of the app.".into());
    }
    if !do_quick && !do_deep {
        return Err("No recovery engine is available. Please reinstall the app.".into());
    }
    if want_quick && !do_quick && do_deep {
        notes.push("This drive's file system doesn't keep deleted file names, so only Deep Scan ran.".into());
    }

    // The runner works inside the save folder, so image paths must be absolute.
    let mut opts = opts;
    if Path::new(&opts.device).is_relative() && Path::new(&opts.device).exists() {
        if let Ok(abs) = fs::canonicalize(&opts.device) {
            opts.device = abs.to_string_lossy().to_string();
        }
    }

    // A drive that is almost full (security cameras and dashcams fill their
    // cards with pre-made recording files) has no free space worth searching:
    // search all of it instead, or the Deep Scan finds nothing.
    if do_deep && !opts.whole_area {
        let (free, total) = opts
            .source_mounts
            .iter()
            .filter_map(|m| disk_space(m))
            .fold((0u64, 0u64), |(f, t), (f2, t2)| (f + f2, t + t2));
        if total > 0 && free * 10 < total {
            opts.whole_area = true;
            notes.push(
                "This drive is almost full (security cameras and dashcams fill their cards like this), \
so RecovMe searched the whole drive instead of only its free space."
                    .into(),
            );
        }
    }

    // ── Output folder checks ──
    let session = new_session(&opts)?;
    let work = session.join(".recovme");
    let quick_dir = work.join("quick");
    let deep_dir = work.join("deep");
    fs::create_dir_all(&quick_dir).map_err(|e| format!("Can't write to the save folder: {e}"))?;
    fs::create_dir_all(&deep_dir).map_err(|e| format!("Can't write to the save folder: {e}"))?;
    let status_file = work.join("status");
    let stop_file = work.join("stop");
    let _ = fs::remove_file(&status_file);
    let _ = fs::remove_file(&stop_file);

    // ── Build the engine commands ──
    let mut steps: Vec<Step> = Vec::new();
    if do_quick {
        steps.push(Step {
            phase: "quick",
            program: engines.tsk_recover.clone().unwrap(),
            args: vec![opts.device.clone(), path_arg(&quick_dir, false)],
            out: work.join("quick.out"),
        });
    }
    if do_deep {
        let area = if opts.whole_area { "wholespace" } else { "freespace" };
        let cmdline = format!(
            "{area},options,keep_corrupted_file_no,{},search",
            photorec_fileopt(&opts.categories)
        );
        steps.push(Step {
            phase: "deep",
            program: engines.photorec.clone().unwrap(),
            args: vec![
                "/log".into(),
                "/d".into(),
                path_arg(&deep_dir.join("recup"), true),
                "/cmd".into(),
                opts.device.clone(),
                cmdline,
            ],
            out: work.join("deep.out"),
        });
    }

    // ── Protect the drive (macOS): unmount so the system stops writing to it ──
    let (remount, unmount_note) = unmount_if_wanted(&opts);
    notes.extend(unmount_note);

    // ── Launch ──
    let stop_flag = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let launch_failed: Arc<std::sync::Mutex<Option<String>>> = Arc::new(Default::default());
    emit(ScanEvent::Progress {
        phase: "starting".into(),
        label: "Getting ready…".into(),
        percent: None,
        files_found: 0,
        eta_secs: None,
        pass: None,
        detail: "Asking for permission to read the drive (read-only).".into(),
    });
    if let Err(e) = launch(&steps, &work, &status_file, &stop_file, stop_flag.clone(), launch_failed.clone()) {
        remount_now(&remount);
        remove_if_empty(&session);
        return Err(e);
    }

    // ── Monitor ──
    let job = Job {
        session: session.clone(),
        stop_file: stop_file.clone(),
        stop_flag: stop_flag.clone(),
        finished: finished.clone(),
    };
    let org = OrgOptions {
        skip_small: opts.skip_small,
        dedupe: opts.dedupe,
        rename_photos: opts.rename_photos,
        categories: opts.categories.clone(),
        // Searching the whole drive also finds files that were never deleted;
        // leave those out by comparing with what is still on the drive.
        existing_roots: if opts.whole_area {
            opts.source_mounts.iter().map(PathBuf::from).collect()
        } else {
            Vec::new()
        },
    };
    let label = opts.device_label.clone();
    let whole_disk = opts.is_whole_disk;
    std::thread::spawn(move || {
        let t0 = Instant::now();
        let mut tracker = DeepTracker::default();
        let mut screen = Screen::default();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut last_files = Instant::now() - Duration::from_secs(10);
        let mut waiting_since: Option<Instant> = None;
        let fail_msg: Option<String>;
        loop {
            std::thread::sleep(Duration::from_millis(700));
            if let Some(msg) = launch_failed.lock().unwrap().clone() {
                remount_now(&remount);
                remove_if_empty(&session);
                emit(ScanEvent::Error { message: msg });
                finished.store(true, Ordering::SeqCst);
                return;
            }
            let st = read_status(&status_file);
            if !st.started {
                // The runner starts as soon as permission is given.
                let w = waiting_since.get_or_insert_with(Instant::now);
                if w.elapsed() > Duration::from_secs(600) {
                    // Make sure a late password entry can't start an unwatched scan.
                    let _ = fs::write(&stop_file, b"stop");
                    remount_now(&remount);
                    remove_if_empty(&session);
                    emit(ScanEvent::Error { message: "The scan didn't start. Please try again.".into() });
                    finished.store(true, Ordering::SeqCst);
                    return;
                }
                continue;
            }
            let quick_count = count_tree(&quick_dir);
            let (deep_count, deep_dirs) = tracker.scan(&deep_dir);
            let files_found = quick_count + deep_count;
            let stopping = stop_flag.load(Ordering::SeqCst) || stop_file.exists();
            let ev = match st.phase.as_deref() {
                Some("quick") => ScanEvent::Progress {
                    phase: "quick".into(),
                    label: "Quick Scan — finding deleted files with their names".into(),
                    percent: None,
                    files_found,
                    eta_secs: None,
                    pass: None,
                    detail: if stopping { "Stopping…".into() } else { "Reading the drive's file table".into() },
                },
                Some("deep") => {
                    screen.feed_file(&work.join("deep.out"));
                    let p = parse_deep(&screen.text());
                    let percent = p.as_ref().and_then(|p| {
                        if p.total > 0 { Some((p.sector as f32 / p.total as f32 * 100.0).min(100.0)) } else { None }
                    });
                    ScanEvent::Progress {
                        phase: "deep".into(),
                        label: "Deep Scan — searching every block for lost files".into(),
                        percent,
                        files_found,
                        eta_secs: p.as_ref().and_then(|p| p.eta_secs),
                        pass: p.as_ref().map(|p| p.pass),
                        detail: if stopping {
                            "Stopping…".into()
                        } else if let Some(p) = &p {
                            format!("Pass {} · {} of {} read", p.pass, human_size(p.sector * 512), human_size(p.total * 512))
                        } else {
                            "Starting the deep search".into()
                        },
                    }
                }
                _ => ScanEvent::Progress {
                    phase: "starting".into(),
                    label: "Starting…".into(),
                    percent: None,
                    files_found,
                    eta_secs: None,
                    pass: None,
                    detail: String::new(),
                },
            };
            emit(ev);

            if last_files.elapsed() > Duration::from_millis(1800) || st.done {
                last_files = Instant::now();
                let mut fresh = new_previews(&quick_dir, &mut seen, true);
                // Only the two newest PhotoRec folders can have new files.
                for (_, d) in deep_dirs.iter().rev().take(2) {
                    fresh.extend(new_previews(d, &mut seen, false));
                }
                if !fresh.is_empty() {
                    fresh.truncate(48);
                    emit(ScanEvent::Files { files: fresh });
                }
            }

            if st.done || st.fail.is_some() {
                fail_msg = st.fail.clone();
                break;
            }
        }

        // 3) Remember whether the user stopped the scan before organizing starts.
        let stopped = stop_flag.load(Ordering::SeqCst) || stop_file.exists();
        remount_now(&remount);
        if let Some(f) = fail_msg {
            remove_if_empty(&session);
            finished.store(true, Ordering::SeqCst);
            emit(ScanEvent::Error {
                message: format!(
                    "The scan couldn't use the save folder ({f}). Choose a folder on this computer, not a network or cloud drive."
                ),
            });
            return;
        }
        let total_raw = count_tree(&quick_dir) + tracker.scan(&deep_dir).0;
        let em = emit.clone();
        let progress = move |done: u64, total: u64| {
            em(ScanEvent::Progress {
                phase: "organize".into(),
                label: "Organizing your files".into(),
                percent: if total > 0 { Some(done as f32 / total as f32 * 100.0) } else { None },
                files_found: total,
                eta_secs: None,
                pass: None,
                detail: "Sorting by type, naming photos by date, removing duplicates".into(),
            });
        };
        progress(0, total_raw);
        let mut summary = organize(&session, &work, &org, &progress);
        summary.stopped = stopped;
        summary.elapsed_secs = t0.elapsed().as_secs();
        summary.notes = notes;

        // Explain engine problems in plain words instead of silently showing "0 files".
        let st = read_status(&status_file);
        let mut failures = Vec::new();
        if !summary.stopped {
            for (phase, rc) in &st.ended {
                if *rc == 0 {
                    continue;
                }
                let reason = engine_error(&work.join(format!("{phase}.out")));
                match phase.as_str() {
                    "quick" => summary.notes.push(
                        if whole_disk && reason.as_deref().is_some_and(|r| r.contains("file system type")) {
                            "Quick Scan reads one volume at a time, so it skipped this whole drive. \
To get original names, scan a volume listed under the drive. Deep Scan still searched all of it."
                                .to_string()
                        } else {
                            format!(
                                "Quick Scan couldn't read this drive's file table{}. Deep Scan still searched it.",
                                reason.as_deref().map(|r| format!(" ({r})")).unwrap_or_default()
                            )
                        },
                    ),
                    _ => failures.push(reason.unwrap_or_else(|| "the drive could not be read".into())),
                }
            }
        }
        if summary.total == 0 && !failures.is_empty() {
            write_report(&session, &summary, &label);
            finished.store(true, Ordering::SeqCst);
            emit(ScanEvent::Error {
                message: format!(
                    "The scan couldn't read the drive: {}. Make sure it is still connected, then try again.{}",
                    failures[0],
                    if cfg!(target_os = "macos") {
                        " If it keeps happening, allow RecovMe in System Settings → Privacy & Security → Full Disk Access."
                    } else {
                        ""
                    }
                ),
            });
            return;
        }
        if summary.total > 0 && !failures.is_empty() {
            summary.notes.push(format!(
                "Deep Scan stopped because of a problem ({}). The files below were found before that.",
                failures[0]
            ));
        }
        if summary.total == 0 && !summary.stopped {
            summary.notes.push(
                "Nothing was found. If the drive was formatted, try again with “Search the whole drive” turned on."
                    .into(),
            );
        }
        if summary.erased > 0 {
            summary.notes.push(format!(
                "{} file{} came back with {} name but no content: the drive had already erased {}. This is common on SSDs. {} in the “{}” folder.",
                summary.erased,
                if summary.erased == 1 { "" } else { "s" },
                if summary.erased == 1 { "its" } else { "their" },
                if summary.erased == 1 { "it" } else { "them" },
                if summary.erased == 1 { "It is" } else { "They are" },
                super::organize::ERASED_FOLDER
            ));
        }
        write_report(&session, &summary, &label);
        finished.store(true, Ordering::SeqCst);
        emit(ScanEvent::Done(summary));
    });
    Ok(job)
}

/// Checks the save folder and picks a fresh "Recovered - <drive> - <date>" folder in it
/// (not created yet).
pub(crate) fn new_session(opts: &ScanOptions) -> Result<PathBuf, String> {
    let mut out_root = PathBuf::from(&opts.output_dir);
    if out_root.is_relative() {
        if let Ok(abs) = fs::canonicalize(&out_root) {
            out_root = abs;
        }
    }
    if !out_root.is_dir() {
        return Err("The save folder doesn't exist. Please pick another one.".into());
    }
    check_output_not_on_source(
        &opts.output_dir,
        &opts.device,
        opts.bsd_name.as_deref(),
        &opts.source_mounts,
    )?;
    let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M");
    let base = format!("Recovered - {} - {stamp}", safe_name(&opts.device_label));
    let mut session = out_root.join(&base);
    let mut n = 2;
    while session.exists() {
        session = out_root.join(format!("{base} ({n})"));
        n += 1;
    }
    Ok(session)
}

/// Remove a session folder that holds nothing but the engines' work files.
pub(crate) fn remove_if_empty(session: &Path) {
    let has_results = fs::read_dir(session)
        .map(|it| it.flatten().any(|e| e.file_name() != ".recovme"))
        .unwrap_or(true);
    if !has_results {
        let _ = fs::remove_dir_all(session);
    }
}

/// First meaningful error line from an engine's output.
fn engine_error(out: &Path) -> Option<String> {
    let text = tail(out, 16 * 1024);
    text.split(['\n', '\r'])
        .map(|l| l.trim())
        .find(|l| {
            let low = l.to_ascii_lowercase();
            (low.contains("unable to") || low.contains("cannot") || low.contains("error") || low.contains("permission denied"))
                && l.len() < 200
        })
        .map(|l| l.trim_end_matches('.').to_string())
}

/// PhotoRec on Windows is a Cygwin program: give it forward slashes.
fn path_arg(p: &Path, for_photorec: bool) -> String {
    let s = p.to_string_lossy().to_string();
    if cfg!(windows) && for_photorec {
        s.replace('\\', "/")
    } else {
        s
    }
}

fn new_previews(dir: &Path, seen: &mut HashSet<PathBuf>, recursive: bool) -> Vec<FoundFile> {
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(dir).max_depth(if recursive { 64 } else { 1 });
    for e in walker.into_iter().filter_map(|e| e.ok()) {
        if !e.file_type().is_file() {
            continue;
        }
        let p = e.path().to_path_buf();
        if seen.contains(&p) {
            continue;
        }
        let ext = p.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let size = e.metadata().map(|m| m.len()).unwrap_or(0);
        // Wait until a file looks complete and is big enough to be a real photo.
        if !previewable(&ext) || size < 20 * 1024 {
            continue;
        }
        seen.insert(p.clone());
        out.push(FoundFile {
            name: e.file_name().to_string_lossy().to_string(),
            path: p.to_string_lossy().to_string(),
            category: "photos".into(),
            size,
        });
    }
    out
}

// ───────────── unmount / remount (macOS) ─────────────

#[derive(Clone)]
struct Remount {
    #[allow(dead_code)]
    bsd: Option<String>,
    #[allow(dead_code)]
    whole: bool,
}

/// macOS: unmount every volume on the physical disk being scanned (this also covers
/// APFS volumes, which live on a separate synthesized disk). Saving to the same
/// physical disk is refused earlier, so nothing the app needs is unmounted.
fn unmount_if_wanted(opts: &ScanOptions) -> (Remount, Option<String>) {
    #[cfg(target_os = "macos")]
    if opts.unmount {
        if let Some(b) = &opts.bsd_name {
            let whole = whole_disk_id(b);
            let out = cmd("diskutil").args(["unmountDisk", &whole]).output();
            return match out {
                Ok(o) if o.status.success() => (Remount { bsd: Some(whole), whole: true }, None),
                _ => (
                    Remount { bsd: None, whole: false },
                    Some("The drive couldn't be unmounted (something may be using it), so it was scanned while mounted. Avoid copying anything onto it.".into()),
                ),
            };
        }
    }
    let _ = opts;
    (Remount { bsd: None, whole: false }, None)
}

#[cfg(target_os = "macos")]
fn whole_disk_id(id: &str) -> String {
    let id = id.trim_start_matches("/dev/").trim_start_matches('r');
    match id.strip_prefix("disk").and_then(|rest| rest.find('s').map(|i| 4 + i)) {
        Some(cut) => id[..cut].to_string(),
        None => id.to_string(),
    }
}

fn remount_now(r: &Remount) {
    #[cfg(target_os = "macos")]
    if let Some(b) = &r.bsd {
        let verb = if r.whole { "mountDisk" } else { "mount" };
        let _ = cmd("diskutil").args([verb, b]).output();
    }
    let _ = r;
}

// ───────────── runners ─────────────

#[cfg(unix)]
fn runner_script(steps: &[Step], work: &Path, status: &Path, stop: &Path) -> String {
    let uid = unsafe { libc::getuid() };
    let gid = unsafe { libc::getgid() };
    let mut s = String::from("#!/bin/sh\n# RecovMe scan runner (read-only on the source drive)\n");
    s.push_str(&format!("W={}\nS={}\nSTOP={}\n", shq(&work.to_string_lossy()), shq(&status.to_string_lossy()), shq(&stop.to_string_lossy())));
    s.push_str(
        r#"echo START > "$S"
run_step() {
  phase="$1"; out="$2"; shift 2
  echo "PHASE $phase" >> "$S"
  "$@" </dev/null >"$out" 2>&1 &
  pid=$!
  while kill -0 "$pid" 2>/dev/null; do
    if [ -f "$STOP" ]; then kill "$pid" 2>/dev/null; sleep 2; kill -9 "$pid" 2>/dev/null; break; fi
    sleep 1
  done
  wait "$pid"; rc=$?
  echo "END $phase $rc" >> "$S"
}
cd "$W" || { echo "FAIL cannot open work folder" >> "$S"; exit 1; }
export TERM=xterm-256color
"#,
    );
    if cfg!(target_os = "macos") {
        // A downloaded app is quarantined; clear it on the engines so macOS lets them run.
        for st in steps {
            s.push_str(&format!("/usr/bin/xattr -d com.apple.quarantine {} 2>/dev/null\n", shq(&st.program.to_string_lossy())));
        }
    }
    for st in steps {
        let mut line = format!("[ -f \"$STOP\" ] || run_step {} {} {}", st.phase, shq(&st.out.to_string_lossy()), shq(&st.program.to_string_lossy()));
        for a in &st.args {
            line.push(' ');
            line.push_str(&shq(a));
        }
        s.push_str(&line);
        s.push('\n');
    }
    if uid != 0 {
        s.push_str(&format!("chown -R {uid}:{gid} \"$W\" 2>/dev/null\n"));
    }
    s.push_str("echo DONE >> \"$S\"\n");
    s
}

#[cfg(unix)]
fn launch(
    steps: &[Step],
    work: &Path,
    status: &Path,
    stop: &Path,
    _stop_flag: Arc<AtomicBool>,
    launch_failed: Arc<std::sync::Mutex<Option<String>>>,
) -> Result<(), String> {
    // The script lives in a private temp folder without spaces, so it is easy to quote.
    let tmp = std::env::temp_dir().join(format!("recovme-{}-{}", std::process::id(), chrono::Local::now().timestamp_millis()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let script = tmp.join("run.sh");
    fs::write(&script, runner_script(steps, work, status, stop)).map_err(|e| e.to_string())?;
    let script_s = script.to_string_lossy().to_string();
    let is_root = unsafe { libc::geteuid() } == 0;

    #[cfg(target_os = "macos")]
    let mut child_cmd = {
        if is_root {
            let mut c = cmd("/bin/sh");
            c.arg(&script_s);
            c
        } else {
            let esc = shq(&script_s).replace('\\', "\\\\").replace('"', "\\\"");
            let apple = format!(
                "do shell script \"/bin/sh {esc} >/dev/null 2>&1 &\" with administrator privileges with prompt \"RecovMe needs your password to read the drive. Nothing on the drive will be changed.\""
            );
            let mut c = cmd("/usr/bin/osascript");
            c.args(["-e", &apple]);
            c
        }
    };
    #[cfg(not(target_os = "macos"))]
    let mut child_cmd = {
        if is_root {
            let mut c = cmd("/bin/sh");
            c.arg(&script_s);
            c
        } else {
            let mut c = cmd("pkexec");
            c.args(["/bin/sh", &script_s]);
            c
        }
    };
    child_cmd.stdin(std::process::Stdio::null());
    child_cmd.stdout(std::process::Stdio::null());
    child_cmd.stderr(std::process::Stdio::piped());
    let child = child_cmd.spawn().map_err(|e| format!("Couldn't start the scan: {e}"))?;
    std::thread::spawn(move || {
        let out = child.wait_with_output();
        match out {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).to_string();
                let msg = if err.contains("-128") || err.to_lowercase().contains("cancel") || o.status.code() == Some(126) {
                    "Permission wasn't given. Your password is needed so the app can read the drive (it never changes anything on it).".to_string()
                } else {
                    format!("Couldn't start the scan: {}", err.trim())
                };
                *launch_failed.lock().unwrap() = Some(msg);
            }
            Err(e) => *launch_failed.lock().unwrap() = Some(format!("Couldn't start the scan: {e}")),
        }
    });
    Ok(())
}

#[cfg(windows)]
fn launch(
    steps: &[Step],
    work: &Path,
    status: &Path,
    stop: &Path,
    stop_flag: Arc<AtomicBool>,
    _launch_failed: Arc<std::sync::Mutex<Option<String>>>,
) -> Result<(), String> {
    use std::io::Write;
    let steps: Vec<(String, PathBuf, Vec<String>, PathBuf)> = steps
        .iter()
        .map(|s| (s.phase.to_string(), s.program.clone(), s.args.clone(), s.out.clone()))
        .collect();
    let work = work.to_path_buf();
    let status = status.to_path_buf();
    let stop = stop.to_path_buf();
    let append = move |line: &str| {
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&status) {
            let _ = writeln!(f, "{line}");
        }
    };
    std::thread::spawn(move || {
        append("START");
        for (phase, program, args, out) in steps {
            if stop_flag.load(Ordering::SeqCst) || stop.exists() {
                break;
            }
            append(&format!("PHASE {phase}"));
            let Ok(outf) = fs::File::create(&out) else {
                append(&format!("END {phase} -1"));
                continue;
            };
            let errf = outf.try_clone().ok();
            let mut c = cmd(&program.to_string_lossy());
            c.args(&args).current_dir(&work).stdin(std::process::Stdio::null()).stdout(outf);
            if let Some(e) = errf {
                c.stderr(e);
            }
            if let Some(dir) = program.parent() {
                // Cygwin / TSK DLLs sit next to the engine.
                let path = std::env::var("PATH").unwrap_or_default();
                c.env("PATH", format!("{};{}", dir.to_string_lossy(), path));
            }
            match c.spawn() {
                Ok(mut child) => {
                    let rc = loop {
                        if stop_flag.load(Ordering::SeqCst) {
                            let _ = child.kill();
                            let _ = child.wait();
                            break -1;
                        }
                        match child.try_wait() {
                            Ok(Some(s)) => break s.code().unwrap_or(-1),
                            Ok(None) => std::thread::sleep(Duration::from_millis(400)),
                            Err(_) => break -1,
                        }
                    };
                    append(&format!("END {phase} {rc}"));
                }
                Err(e) => append(&format!("END {phase} -1 {e}")),
            }
        }
        append("DONE");
    });
    Ok(())
}

/// (free bytes, total bytes) of the volume mounted at `path`.
#[cfg(unix)]
fn disk_space(path: &str) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let unit = st.f_frsize as u64;
    Some((st.f_bfree as u64 * unit, st.f_blocks as u64 * unit))
}

#[cfg(windows)]
fn disk_space(path: &str) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    }
    let wide: Vec<u16> = std::ffi::OsStr::new(path).encode_wide().chain(Some(0)).collect();
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) };
    (ok != 0).then_some((free, total))
}
