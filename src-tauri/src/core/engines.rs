//! Locates the bundled recovery engines:
//!   - PhotoRec (CGSecurity) — Deep Scan, signature carving, 480+ file types
//!   - tsk_recover (The Sleuth Kit) — Quick Scan, undelete with original names
//! Bundled under resources/engines/{photorec,tsk}/…; falls back to PATH for development.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Engines {
    pub photorec: Option<PathBuf>,
    pub tsk_recover: Option<PathBuf>,
}

#[cfg(windows)]
const PHOTOREC_NAMES: &[&str] = &["photorec_win.exe", "photorec.exe"];
#[cfg(not(windows))]
const PHOTOREC_NAMES: &[&str] = &["photorec"];
#[cfg(windows)]
const TSK_NAMES: &[&str] = &["tsk_recover.exe"];
#[cfg(not(windows))]
const TSK_NAMES: &[&str] = &["tsk_recover"];

fn find_in(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    walkdir::WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_type().is_file()
                && names
                    .iter()
                    .any(|n| e.file_name().to_string_lossy().eq_ignore_ascii_case(n))
        })
        .map(|e| e.into_path())
}

fn find_on_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    // Homebrew locations, in case the app was launched from Finder without a shell PATH.
    dirs.push("/opt/homebrew/bin".into());
    dirs.push("/usr/local/bin".into());
    dirs.push("/usr/bin".into());
    for d in dirs {
        for n in names {
            let p = d.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// `resource_root` is the folder that contains `photorec/` and `tsk/`.
pub fn locate(resource_root: Option<&Path>) -> Engines {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(dev) = std::env::var("RECOVME_ENGINES") {
        roots.push(dev.into());
    }
    if let Some(r) = resource_root {
        roots.push(r.to_path_buf());
    }
    let mut e = Engines::default();
    for r in &roots {
        if e.photorec.is_none() {
            e.photorec = find_in(&r.join("photorec"), PHOTOREC_NAMES);
        }
        if e.tsk_recover.is_none() {
            e.tsk_recover = find_in(&r.join("tsk"), TSK_NAMES);
        }
    }
    if e.photorec.is_none() {
        e.photorec = find_on_path(PHOTOREC_NAMES);
    }
    if e.tsk_recover.is_none() {
        e.tsk_recover = find_on_path(TSK_NAMES);
    }
    #[cfg(unix)]
    for p in [&e.photorec, &e.tsk_recover].into_iter().flatten() {
        // Bundled resources can lose the executable bit.
        use std::os::unix::fs::PermissionsExt;
        if let Ok(m) = std::fs::metadata(p) {
            if m.permissions().mode() & 0o111 == 0 {
                let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
            }
        }
    }
    e
}
