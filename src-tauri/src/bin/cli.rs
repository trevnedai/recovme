//! Headless runner that uses exactly the same scan code as the app.
//! Usage: recovme-cli <device-or-image> <save-folder> [smart|quick|deep] [categories,comma,separated] [--whole] [--mount DIR]
//!        recovme-cli adb:<serial> <save-folder> - [categories] [--places trash,thumbnails,apps,all]
//!        recovme-cli iosbackup:<backup folder> <save-folder> - [categories] [--password PW]
//!        recovme-cli --drives | --phones

use recovme_lib::core::{devices, drives, engines, job, types::*};
use std::sync::mpsc;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("--drives") {
        println!("{}", serde_json::to_string_pretty(&drives::list_drives()).unwrap());
        return;
    }
    if args.first().map(|s| s.as_str()) == Some("--phones") {
        println!("{}", serde_json::to_string_pretty(&devices::list(&engines::locate(None))).unwrap());
        return;
    }
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned());
    if args.len() < 2 {
        eprintln!("usage: recovme-cli <device-or-image> <save-folder> [smart|quick|deep] [photos,videos,...] [--whole]");
        std::process::exit(2);
    }
    let mode = match args.get(2).map(|s| s.as_str()) {
        Some("quick") => ScanMode::Quick,
        Some("deep") => ScanMode::Deep,
        _ => ScanMode::Smart,
    };
    let categories: Vec<String> = args
        .get(3)
        .filter(|s| !s.starts_with("--"))
        .map(|s| s.split(',').map(|x| x.to_string()).collect())
        .unwrap_or_else(|| vec!["everything".into()]);
    let image = devices::picked(std::path::Path::new(&args[0]))
        .or_else(|| {
            devices::is_device(&args[0])
                .then(|| devices::list(&engines::locate(None)).into_iter().find(|d| d.device == args[0]))
                .flatten()
        })
        .or_else(|| drives::image_drive(&args[0]).ok());
    // A backup folder given by path is read as a backup.
    let device = image
        .as_ref()
        .filter(|d| d.kind == "ios-backup")
        .map(|d| d.device.clone())
        .unwrap_or_else(|| args[0].clone());
    let opts = ScanOptions {
        device,
        device_label: image.as_ref().map(|d| d.label.clone()).unwrap_or_else(|| args[0].clone()),
        bsd_name: None,
        filesystem: None,
        is_whole_disk: true,
        // --mount DIR: pretend the drive is mounted there (to test "still on the drive").
        source_mounts: flag("--mount")
            .map(|m| vec![m])
            .unwrap_or_else(|| image.as_ref().map(|d| d.mount_points.clone()).unwrap_or_default()),
        mode,
        whole_area: args.iter().any(|a| a == "--whole"),
        categories,
        output_dir: args[1].clone(),
        skip_small: true,
        dedupe: true,
        rename_photos: true,
        unmount: false,
        places: flag("--places").map(|p| p.split(',').map(|x| x.to_string()).collect()).unwrap_or_default(),
        password: flag("--password"),
    };
    let eng = engines::locate(None);
    eprintln!("engines: {eng:?}");
    let (tx, rx) = mpsc::channel::<ScanEvent>();
    let emit: job::Emit = Arc::new(move |e| {
        let _ = tx.send(e);
    });
    let j = match job::start(opts, eng, emit) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("session: {}", j.session.display());
    for ev in rx {
        let line = serde_json::to_string(&ev).unwrap();
        println!("{line}");
        if matches!(ev, ScanEvent::Done(_) | ScanEvent::Error { .. }) {
            break;
        }
    }
}
