//! Headless runner that uses exactly the same scan code as the app.
//! Usage: recovme-cli <device-or-image> <save-folder> [smart|quick|deep] [categories,comma,separated] [--whole]
//!        recovme-cli --drives

use recovme_lib::core::{drives, engines, job, types::*};
use std::sync::mpsc;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("--drives") {
        println!("{}", serde_json::to_string_pretty(&drives::list_drives()).unwrap());
        return;
    }
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
    let image = drives::image_drive(&args[0]).ok();
    let opts = ScanOptions {
        device: args[0].clone(),
        device_label: image.map(|d| d.label).unwrap_or_else(|| args[0].clone()),
        bsd_name: None,
        filesystem: None,
        is_whole_disk: true,
        source_mounts: vec![],
        mode,
        whole_area: args.iter().any(|a| a == "--whole"),
        categories,
        output_dir: args[1].clone(),
        skip_small: true,
        dedupe: true,
        rename_photos: true,
        unmount: false,
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
