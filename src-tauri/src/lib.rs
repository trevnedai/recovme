// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
pub mod core;

use crate::core::devices;
use crate::core::drives;
use crate::core::engines::{self, Engines};
use crate::core::job::{self, Job};
use crate::core::types::{DiskGroup, Drive, ScanEvent, ScanOptions};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

#[derive(Default)]
struct AppState {
    job: Mutex<Option<Job>>,
}

fn engines_for(app: &AppHandle) -> Engines {
    let res = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("resources").join("engines"));
    engines::locate(res.as_deref())
}

#[tauri::command]
async fn list_drives() -> Result<Vec<DiskGroup>, String> {
    tauri::async_runtime::spawn_blocking(drives::list_drives)
        .await
        .map_err(|e| e.to_string())?
}

/// Android phones (USB debugging) and iPhone / iPad backups on this computer.
#[tauri::command]
async fn list_phones(app: AppHandle) -> Result<Vec<Drive>, String> {
    let engines = engines_for(&app);
    tauri::async_runtime::spawn_blocking(move || devices::list(&engines))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn engine_status(app: AppHandle) -> Engines {
    engines_for(&app)
}

#[tauri::command]
fn platform() -> &'static str {
    std::env::consts::OS
}

#[tauri::command]
async fn default_output_dir(app: AppHandle) -> Option<String> {
    // The home folder, not the Desktop: OneDrive and iCloud often sync the Desktop,
    // which would upload private recovered files to the cloud.
    app.path()
        .home_dir()
        .or_else(|_| app.path().desktop_dir())
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .set_title("Where should recovered files be saved?")
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
async fn pick_image(app: AppHandle) -> Result<Option<Drive>, String> {
    let picked = app
        .dialog()
        .file()
        .set_title("Choose a disk image, or the Manifest.plist file of an iPhone backup")
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok());
    match picked {
        Some(p) => match devices::picked(&p) {
            Some(backup) => Ok(Some(backup)),
            None => drives::image_drive(&p.to_string_lossy()).map(Some),
        },
        None => Ok(None),
    }
}

#[tauri::command]
async fn check_output(opts: ScanOptions) -> Result<(), String> {
    drives::check_output_not_on_source(
        &opts.output_dir,
        &opts.device,
        opts.bsd_name.as_deref(),
        &opts.source_mounts,
    )
}

#[tauri::command]
async fn start_scan(app: AppHandle, state: State<'_, AppState>, opts: ScanOptions) -> Result<String, String> {
    {
        let guard = state.job.lock().unwrap();
        if let Some(j) = guard.as_ref() {
            if !j.is_finished() {
                return Err("A scan is already running.".into());
            }
        }
    }
    let engines = engines_for(&app);
    let handle = app.clone();
    let emit: job::Emit = Arc::new(move |ev: ScanEvent| {
        let _ = handle.emit("scan-event", ev);
    });
    let output_dir = opts.output_dir.clone();
    let j = job::start(opts, engines, emit)?;
    // Let the window show thumbnails from the results folder.
    let _ = app.asset_protocol_scope().allow_directory(&j.session, true);
    let _ = app.asset_protocol_scope().allow_directory(&output_dir, true);
    let session = j.session.to_string_lossy().to_string();
    *state.job.lock().unwrap() = Some(j);
    Ok(session)
}

#[tauri::command]
fn stop_scan(state: State<'_, AppState>) {
    if let Some(j) = state.job.lock().unwrap().as_ref() {
        j.stop();
    }
}

#[tauri::command]
fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

/// A newer version, as announced by the latest release on GitHub.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    version: String,
    current: String,
}

fn scan_running(state: &State<'_, AppState>) -> bool {
    state.job.lock().unwrap().as_ref().is_some_and(|j| !j.is_finished())
}

/// Asks GitHub whether a newer RecovMe exists. Sends nothing but the request
/// itself; files and scans never leave the computer.
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfo { version: u.version.clone(), current: u.current_version.clone() }))
}

/// Downloads the update, checks its signature, installs it and restarts.
/// Refused while a scan runs, so a scan is never cut short.
#[tauri::command]
async fn install_update(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if scan_running(&state) {
        return Err("A scan is running. Update when it has finished.".into());
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("RecovMe is already up to date.")?;
    let handle = app.clone();
    let mut got: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                got += chunk as u64;
                let _ = handle.emit("update-progress", (got, total));
            },
            || {},
        )
        .await
        .map_err(|e| format!("The update couldn't be installed: {e}"))?;
    app.restart();
}

/// Opens one of RecovMe's own web pages (donation, source code) in the browser.
#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    const ALLOWED: [&str; 5] = [
        "https://paypal.me/",
        "https://www.paypal.com/paypalme/",
        "https://github.com/trevnedai/recovme",
        "https://trevnedai.github.io/recovme",
        "https://recovme.app/",
    ];
    if !ALLOWED.iter().any(|p| url.starts_with(p)) {
        return Err("That link isn't allowed.".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    // RECOVME_DEMO=welcome|setup|scan|done opens the app with sample data
    // (used for website screenshots; never set in normal use).
    if let Ok(step) = std::env::var("RECOVME_DEMO") {
        let script = format!(
            "window.__RECOVME_DEMO__ = {};",
            serde_json::to_string(&step).unwrap_or_else(|_| "\"welcome\"".into())
        );
        builder = builder.plugin(
            tauri::plugin::Builder::<tauri::Wry>::new("recovme-demo")
                .js_init_script(script)
                .build(),
        );
    }
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_drives,
            list_phones,
            engine_status,
            platform,
            default_output_dir,
            pick_folder,
            pick_image,
            check_output,
            start_scan,
            stop_scan,
            open_path,
            open_url,
            check_update,
            install_update
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if stop_then_exit(window.app_handle()) {
                    api.prevent_close();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while starting RecovMe")
        .run(|app, event| {
            // Cmd+Q / quitting from the Dock: same as closing the window.
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = &event {
                if stop_then_exit(app) {
                    api.prevent_exit();
                }
            }
        });
}

static CLOSING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// If a scan is running: stop it, let it save and sort what it found (and remount the
/// drive), then quit. Returns true when quitting has to wait for that.
fn stop_then_exit(app: &AppHandle) -> bool {
    use std::sync::atomic::Ordering;
    let running = {
        let state = app.state::<AppState>();
        let guard = state.job.lock().unwrap();
        match guard.as_ref() {
            Some(j) if !j.is_finished() => {
                j.stop();
                true
            }
            _ => false,
        }
    };
    if !running {
        return false;
    }
    if CLOSING.swap(true, Ordering::SeqCst) {
        return true; // already waiting
    }
    let _ = app.emit("app-closing", ());
    let handle = app.clone();
    std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let done = {
                let state = handle.state::<AppState>();
                let guard = state.job.lock().unwrap();
                guard.as_ref().map(|j| j.is_finished()).unwrap_or(true)
            };
            if done || t0.elapsed() > std::time::Duration::from_secs(900) {
                break;
            }
        }
        handle.exit(0);
    });
    true
}
