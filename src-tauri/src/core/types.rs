// RecovMe — Copyright © 2026 Dhan Denvert Asuncion
// SPDX-License-Identifier: GPL-3.0-or-later (with additional terms: see NOTICE.md)
use serde::{Deserialize, Serialize};

/// A drive, partition/volume, or disk image the user can scan.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    /// Stable id for the UI.
    pub id: String,
    /// Main label, e.g. "SanDisk Ultra" or "NO NAME".
    pub label: String,
    /// Second line, e.g. "USB · FAT32 · mounted at /Volumes/NO NAME".
    pub detail: String,
    /// Path handed to the engines (/dev/rdisk4s1, \\.\E:, /path/card.img).
    pub device: String,
    /// macOS identifier (disk4s1) used for unmount/mount. None elsewhere.
    pub bsd_name: Option<String>,
    pub size: u64,
    /// "internal" | "external" | "removable" | "image" | "android" | "ios-backup"
    pub kind: String,
    pub solid_state: Option<bool>,
    pub filesystem: Option<String>,
    /// Where this volume (or any volume on this disk) is mounted.
    pub mount_points: Vec<String>,
    pub is_whole_disk: bool,
    pub is_system: bool,
    /// Plain-language warning about recovery chances, if any.
    pub warning: Option<String>,
    /// Quick Scan (original names) can work on this file system.
    pub quick_supported: bool,
    /// Why this source can't be scanned yet (a phone that hasn't allowed USB debugging).
    pub blocked: Option<String>,
    /// An encrypted iPhone backup: needs its password.
    pub encrypted: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiskGroup {
    pub disk: Drive,
    pub volumes: Vec<Drive>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    /// Quick Scan first (original names), then Deep Scan (signatures).
    Smart,
    Quick,
    Deep,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    pub device: String,
    pub device_label: String,
    pub bsd_name: Option<String>,
    pub filesystem: Option<String>,
    pub is_whole_disk: bool,
    /// Mount points of the drive being scanned (output must not go there).
    pub source_mounts: Vec<String>,
    pub mode: ScanMode,
    /// Search the whole drive, not only free space (after a format / lost partition).
    pub whole_area: bool,
    /// "photos" | "videos" | "audio" | "documents" | "archives" | "everything"
    pub categories: Vec<String>,
    pub output_dir: String,
    pub skip_small: bool,
    pub dedupe: bool,
    pub rename_photos: bool,
    /// macOS: unmount the volume while scanning so the system can't overwrite deleted data.
    pub unmount: bool,
    /// Android: where to look ("trash", "thumbnails", "apps", "all").
    #[serde(default)]
    pub places: Vec<String>,
    /// Encrypted iPhone backup: its password. Only kept in memory.
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoundFile {
    pub path: String,
    pub name: String,
    pub category: String,
    pub size: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    pub key: String,
    pub label: String,
    pub count: u64,
    pub bytes: u64,
    pub path: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub session_dir: String,
    pub total: u64,
    pub total_bytes: u64,
    pub categories: Vec<CategoryCount>,
    pub original_names: u64,
    pub original_names_path: String,
    pub duplicates_removed: u64,
    pub small_set_aside: u64,
    /// Files whose content was already wiped (all zeros), set aside.
    pub erased: u64,
    /// Files found by a whole-drive search that are still on the drive, left out.
    pub still_on_drive: u64,
    pub stopped: bool,
    pub elapsed_secs: u64,
    pub sample_photos: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ScanEvent {
    #[serde(rename_all = "camelCase")]
    Progress {
        /// "starting" | "quick" | "deep" | "copy" | "organize"
        phase: String,
        label: String,
        percent: Option<f32>,
        files_found: u64,
        eta_secs: Option<u64>,
        pass: Option<u32>,
        detail: String,
    },
    #[serde(rename_all = "camelCase")]
    Files { files: Vec<FoundFile> },
    Done(Summary),
    #[serde(rename_all = "camelCase")]
    Error { message: String },
}
