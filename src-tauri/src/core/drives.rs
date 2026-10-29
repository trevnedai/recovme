//! Lists drives and volumes on macOS (diskutil), Windows (PowerShell storage
//! cmdlets) and Linux (lsblk), and checks the output folder is on another drive.

use super::categories::quick_supported;
use super::types::{DiskGroup, Drive};
use super::util::{human_size, run_capture};

#[allow(dead_code)]
const TRIM_WARNING: &str = "Internal SSD: deleted files are usually wiped automatically (TRIM). \
Chances are low — check the Trash/Recycle Bin, backups or cloud first.";
#[allow(dead_code)]
const MAC_INTERNAL_WARNING: &str = "Mac internal drive: it is encrypted and uses TRIM, so deleted \
files can't be carved back. Check Trash, Time Machine or iCloud instead.";

pub fn list_drives() -> Result<Vec<DiskGroup>, String> {
    #[cfg(target_os = "macos")]
    return mac::list();
    #[cfg(windows)]
    return win::list();
    #[cfg(target_os = "linux")]
    return linux::list();
    #[allow(unreachable_code)]
    Ok(vec![])
}

/// Describe a disk image file the user picked, so it can be scanned like a drive.
pub fn image_drive(path: &str) -> Result<Drive, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Can't open that file: {e}"))?;
    let name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    Ok(Drive {
        id: format!("image:{path}"),
        label: name,
        detail: format!("Disk image · {}", human_size(meta.len())),
        device: path.to_string(),
        bsd_name: None,
        size: meta.len(),
        kind: "image".into(),
        solid_state: None,
        filesystem: None,
        mount_points: vec![],
        is_whole_disk: true,
        is_system: false,
        warning: None,
        quick_supported: true,
        blocked: None,
        encrypted: false,
    })
}

fn detail_line(parts: &[Option<String>]) -> String {
    parts
        .iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Refuse to save into the drive being scanned: writing there overwrites the
/// very data we are trying to bring back.
pub fn check_output_not_on_source(
    output_dir: &str,
    device: &str,
    bsd_name: Option<&str>,
    source_mounts: &[String],
) -> Result<(), String> {
    let msg = "Please save to a different drive than the one you are scanning — \
writing there could overwrite the files you want back.";
    let out = std::path::Path::new(output_dir);
    let out_canon = std::fs::canonicalize(out).unwrap_or_else(|_| out.to_path_buf());
    for m in source_mounts {
        if m.is_empty() || m == "/" {
            continue;
        }
        // Windows canonicalize() gives \\?\E:\…, which never starts_with("E:\"),
        // so check the path as typed too.
        if out_canon.starts_with(m) || out.starts_with(m) {
            return Err(msg.into());
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let (Some(src), Some(dst)) = (
            bsd_name.map(mac::whole_disk_of),
            mac::physical_disk_of_path(output_dir),
        ) {
            if src == dst {
                return Err(msg.into());
            }
        }
    }
    #[cfg(windows)]
    {
        if let Some(letter) = win::drive_letter_of(output_dir) {
            if device.eq_ignore_ascii_case(&format!("\\\\.\\{letter}:")) {
                return Err(msg.into());
            }
            if let Some(n) = device.strip_prefix("\\\\.\\PhysicalDrive") {
                if let Some(dn) = win::disk_number_of_letter(letter) {
                    if n == dn.to_string() {
                        return Err(msg.into());
                    }
                }
            }
        }
    }
    let _ = (device, bsd_name);
    Ok(())
}

// ───────────────────────────── macOS ─────────────────────────────
#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use plist::{Dictionary, Value};

    fn plist_of(args: &[&str]) -> Result<Dictionary, String> {
        let out = run_capture("diskutil", args)?;
        let v = Value::from_reader_xml(out.as_bytes()).map_err(|e| e.to_string())?;
        v.into_dictionary().ok_or_else(|| "unexpected diskutil output".into())
    }
    fn s(d: &Dictionary, k: &str) -> Option<String> {
        d.get(k).and_then(|v| v.as_string()).map(|s| s.to_string())
    }
    fn b(d: &Dictionary, k: &str) -> Option<bool> {
        d.get(k).and_then(|v| v.as_boolean())
    }
    fn n(d: &Dictionary, k: &str) -> u64 {
        d.get(k)
            .and_then(|v| v.as_unsigned_integer())
            .unwrap_or(0)
    }

    pub fn whole_disk_of(id: &str) -> String {
        // disk4s1 -> disk4
        let id = id.trim_start_matches("/dev/").trim_start_matches('r');
        if !id.starts_with("disk") {
            return id.to_string();
        }
        match id[4..].find('s') {
            Some(i) => id[..4 + i].to_string(),
            None => id.to_string(),
        }
    }

    /// Physical whole disk (e.g. "disk0") holding a path, following APFS to its physical store.
    pub fn physical_disk_of_path(path: &str) -> Option<String> {
        let info = plist_of(&["info", "-plist", path]).ok()?;
        if let Some(stores) = info.get("APFSPhysicalStores").and_then(|v| v.as_array()) {
            if let Some(first) = stores.first().and_then(|v| v.as_dictionary()) {
                if let Some(st) = s(first, "APFSPhysicalStore") {
                    return Some(whole_disk_of(&st));
                }
            }
        }
        s(&info, "ParentWholeDisk").map(|p| whole_disk_of(&p))
    }

    pub fn list() -> Result<Vec<DiskGroup>, String> {
        let root = plist_of(&["list", "-plist", "physical"])?;
        let disks = root
            .get("AllDisksAndPartitions")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut groups = Vec::new();
        for d in disks {
            let Some(d) = d.as_dictionary() else { continue };
            let Some(id) = s(d, "DeviceIdentifier") else { continue };
            let info = plist_of(&["info", "-plist", &id]).unwrap_or_default();
            let removable = b(&info, "RemovableMedia").unwrap_or(false)
                || b(&info, "Removable").unwrap_or(false);
            let ssd = b(&info, "SolidState");
            let bus = s(&info, "BusProtocol").unwrap_or_default();
            // The MacBook's built-in SD slot reports Internal=true; it is still a card.
            let internal = b(&info, "Internal").unwrap_or(false)
                && !removable
                && !bus.contains("Secure Digital");
            let media = s(&info, "MediaName").unwrap_or_else(|| id.clone());
            let size = n(d, "Size").max(n(&info, "Size"));
            let kind = if internal {
                "internal"
            } else if removable || bus.contains("Secure Digital") {
                "removable"
            } else {
                "external"
            };

            let mut volumes = Vec::new();
            let mut all_mounts = Vec::new();
            let parts = d
                .get("Partitions")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for p in parts {
                let Some(p) = p.as_dictionary() else { continue };
                let Some(pid) = s(p, "DeviceIdentifier") else { continue };
                let content = s(p, "Content").unwrap_or_default();
                if matches!(
                    content.as_str(),
                    "EFI" | "Apple_Boot" | "Apple_APFS_ISC" | "Apple_APFS_Recovery"
                        | "Microsoft Reserved"
                ) {
                    continue;
                }
                let pinfo = plist_of(&["info", "-plist", &pid]).unwrap_or_default();
                let fs = s(&pinfo, "FilesystemType").or_else(|| {
                    if content == "Apple_APFS" {
                        Some("apfs".into())
                    } else {
                        None
                    }
                });
                let fs_name = s(&pinfo, "FilesystemUserVisibleName").or_else(|| fs.clone());
                let mount = s(p, "MountPoint").or_else(|| s(&pinfo, "MountPoint"));
                if let Some(m) = &mount {
                    all_mounts.push(m.clone());
                }
                let vname = s(p, "VolumeName")
                    .or_else(|| s(&pinfo, "VolumeName"))
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| {
                        if content == "Apple_APFS" {
                            "APFS container".into()
                        } else {
                            "Untitled partition".into()
                        }
                    });
                let psize = n(p, "Size");
                volumes.push(Drive {
                    id: pid.clone(),
                    label: vname,
                    detail: detail_line(&[
                        Some(human_size(psize)),
                        fs_name,
                        mount.as_ref().map(|m| format!("on {m}")),
                    ]),
                    device: format!("/dev/r{pid}"),
                    bsd_name: Some(pid.clone()),
                    size: psize,
                    kind: kind.into(),
                    solid_state: ssd,
                    quick_supported: quick_supported(fs.as_deref())
                        && fs.as_deref() != Some("apfs"),
                    filesystem: fs,
                    mount_points: mount.into_iter().collect(),
                    is_whole_disk: false,
                    is_system: internal,
                    warning: if internal { Some(MAC_INTERNAL_WARNING.into()) } else { None },
                    blocked: None,
                    encrypted: false,
                });
            }

            // A card formatted without a partition table: the disk is the volume.
            if volumes.is_empty() {
                if let Some(m) = s(d, "MountPoint").or_else(|| s(&info, "MountPoint")) {
                    all_mounts.push(m);
                }
            }
            let fs = s(&info, "FilesystemType");
            let disk = Drive {
                id: id.clone(),
                label: media,
                detail: detail_line(&[
                    Some(human_size(size)),
                    Some(match kind {
                        "internal" => "Internal".to_string(),
                        "removable" => "Card / removable".to_string(),
                        _ => "External".to_string(),
                    }),
                    if bus.is_empty() { None } else { Some(bus.clone()) },
                    ssd.map(|x| if x { "SSD".to_string() } else { "HDD".to_string() }),
                ]),
                device: format!("/dev/r{id}"),
                bsd_name: Some(id.clone()),
                size,
                kind: kind.into(),
                solid_state: ssd,
                quick_supported: volumes.is_empty() && quick_supported(fs.as_deref()),
                filesystem: fs,
                mount_points: all_mounts.clone(),
                is_whole_disk: true,
                is_system: internal,
                warning: if internal { Some(MAC_INTERNAL_WARNING.into()) } else { None },
                blocked: None,
                encrypted: false,
            };
            groups.push(DiskGroup { disk, volumes });
        }
        // External and removable first: that is where recovery works.
        groups.sort_by_key(|g| g.disk.kind == "internal");
        Ok(groups)
    }
}

// ───────────────────────────── Windows ─────────────────────────────
#[cfg(windows)]
mod win {
    use super::*;
    use serde_json::Value;

    const PS: &str = r#"
$ErrorActionPreference='SilentlyContinue'
$disks = @(Get-Disk | ForEach-Object { [pscustomobject]@{
  Number=[int]$_.Number; Name="$($_.FriendlyName)"; Size=[uint64]$_.Size;
  Bus="$($_.BusType)"; System=[bool]($_.IsSystem -or $_.IsBoot) } })
$phys = @(Get-PhysicalDisk | ForEach-Object { [pscustomobject]@{ Id="$($_.DeviceId)"; Media="$($_.MediaType)" } })
$parts = @(Get-Partition | ForEach-Object {
  $l = ''; if ($_.DriveLetter -and [int][char]$_.DriveLetter -ne 0) { $l = "$($_.DriveLetter)" }
  [pscustomobject]@{ Disk=[int]$_.DiskNumber; Num=[int]$_.PartitionNumber; Letter=$l; Size=[uint64]$_.Size; Type="$($_.Type)" } })
$vols = @(Get-Volume | Where-Object { $_.DriveLetter } | ForEach-Object { [pscustomobject]@{
  Letter="$($_.DriveLetter)"; Label="$($_.FileSystemLabel)"; Fs="$($_.FileSystem)" } })
[pscustomobject]@{ disks=$disks; phys=$phys; parts=$parts; vols=$vols } | ConvertTo-Json -Depth 4 -Compress
"#;

    fn arr(v: &Value, k: &str) -> Vec<Value> {
        match v.get(k) {
            Some(Value::Array(a)) => a.clone(),
            Some(Value::Null) | None => vec![],
            Some(other) => vec![other.clone()],
        }
    }
    fn st(v: &Value, k: &str) -> String {
        match v.get(k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        }
    }
    fn num(v: &Value, k: &str) -> u64 {
        v.get(k).and_then(|x| x.as_u64()).unwrap_or(0)
    }
    fn bus_name(b: &str) -> String {
        match b {
            "7" => "USB".into(),
            "17" => "NVMe".into(),
            "11" => "SATA".into(),
            "12" => "SD".into(),
            "13" => "MMC".into(),
            "3" => "ATA".into(),
            "10" => "SAS".into(),
            "8" => "RAID".into(),
            other => other.into(),
        }
    }

    fn ps(script: &str) -> Result<String, String> {
        run_capture(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script],
        )
    }

    pub fn drive_letter_of(path: &str) -> Option<char> {
        let c = path.chars().next()?;
        if path.chars().nth(1) == Some(':') && c.is_ascii_alphabetic() {
            Some(c.to_ascii_uppercase())
        } else {
            None
        }
    }

    pub fn disk_number_of_letter(letter: char) -> Option<u32> {
        ps(&format!("(Get-Partition -DriveLetter {letter}).DiskNumber"))
            .ok()?
            .trim()
            .parse()
            .ok()
    }

    pub fn list() -> Result<Vec<DiskGroup>, String> {
        let raw = ps(PS)?;
        let v: Value = serde_json::from_str(raw.trim()).map_err(|e| format!("drive list: {e}"))?;
        let disks = arr(&v, "disks");
        let phys = arr(&v, "phys");
        let parts = arr(&v, "parts");
        let vols = arr(&v, "vols");
        let mut groups = Vec::new();
        for d in disks {
            let number = num(&d, "Number");
            let bus = bus_name(&st(&d, "Bus"));
            let system = d.get("System").and_then(|x| x.as_bool()).unwrap_or(false);
            let media = phys
                .iter()
                .find(|p| st(p, "Id") == number.to_string())
                .map(|p| st(p, "Media"))
                .unwrap_or_default();
            let ssd = match media.as_str() {
                "SSD" | "4" => Some(true),
                "HDD" | "3" => Some(false),
                _ => None,
            };
            let kind = if bus == "SD" || bus == "MMC" {
                "removable"
            } else if bus == "USB" || bus == "1394" {
                "external"
            } else {
                "internal"
            };
            let trim_warn = kind == "internal" && ssd == Some(true);
            let size = num(&d, "Size");
            let mut volumes = Vec::new();
            let mut mounts = Vec::new();
            for p in parts.iter().filter(|p| num(p, "Disk") == number) {
                let letter = st(p, "Letter");
                let ptype = st(p, "Type");
                if ptype.contains("Reserved") || ptype.contains("System") && letter.is_empty() {
                    continue;
                }
                let psize = num(p, "Size");
                let vol = vols.iter().find(|x| st(x, "Letter") == letter && !letter.is_empty());
                let fs = vol.map(|x| st(x, "Fs")).filter(|s| !s.is_empty());
                let label = vol.map(|x| st(x, "Label")).filter(|s| !s.is_empty());
                if letter.is_empty() {
                    continue; // engines address volumes by letter; whole-disk scan covers the rest
                }
                mounts.push(format!("{letter}:\\"));
                volumes.push(Drive {
                    id: format!("vol:{letter}"),
                    label: match &label {
                        Some(l) => format!("{l} ({letter}:)"),
                        None => format!("Drive {letter}:"),
                    },
                    detail: detail_line(&[Some(human_size(psize)), fs.clone()]),
                    device: format!("\\\\.\\{letter}:"),
                    bsd_name: None,
                    size: psize,
                    kind: kind.into(),
                    solid_state: ssd,
                    quick_supported: quick_supported(fs.as_deref()),
                    filesystem: fs,
                    mount_points: vec![format!("{letter}:\\")],
                    is_whole_disk: false,
                    is_system: system || letter == "C",
                    warning: if trim_warn { Some(TRIM_WARNING.into()) } else { None },
                    blocked: None,
                    encrypted: false,
                });
            }
            groups.push(DiskGroup {
                disk: Drive {
                    id: format!("disk:{number}"),
                    label: st(&d, "Name"),
                    detail: detail_line(&[
                        Some(human_size(size)),
                        Some(match kind {
                            "internal" => "Internal".to_string(),
                            "removable" => "Card / removable".to_string(),
                            _ => "External".to_string(),
                        }),
                        if bus.is_empty() { None } else { Some(bus.clone()) },
                        ssd.map(|x| if x { "SSD".to_string() } else { "HDD".to_string() }),
                    ]),
                    device: format!("\\\\.\\PhysicalDrive{number}"),
                    bsd_name: None,
                    size,
                    kind: kind.into(),
                    solid_state: ssd,
                    filesystem: None,
                    mount_points: mounts,
                    is_whole_disk: true,
                    is_system: system,
                    warning: if trim_warn { Some(TRIM_WARNING.into()) } else { None },
                    // Quick Scan reads one file system; pick a volume for it.
                    quick_supported: volumes.is_empty(),
                    blocked: None,
                    encrypted: false,
                },
                volumes,
            });
        }
        groups.sort_by_key(|g| g.disk.kind == "internal");
        Ok(groups)
    }
}

// ───────────────────────────── Linux ─────────────────────────────
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use serde_json::Value;

    fn st(v: &Value, k: &str) -> Option<String> {
        v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string())
    }
    fn bo(v: &Value, k: &str) -> bool {
        match v.get(k) {
            Some(Value::Bool(b)) => *b,
            Some(Value::String(s)) => s == "1",
            Some(Value::Number(n)) => n.as_u64() == Some(1),
            _ => false,
        }
    }
    fn sz(v: &Value) -> u64 {
        match v.get("size") {
            Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
            Some(Value::String(s)) => s.parse().unwrap_or(0),
            _ => 0,
        }
    }

    pub fn list() -> Result<Vec<DiskGroup>, String> {
        let out = run_capture(
            "lsblk",
            &["-J", "-b", "-o", "NAME,PATH,SIZE,TYPE,FSTYPE,LABEL,MOUNTPOINT,ROTA,RM,TRAN,MODEL"],
        )?;
        let v: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
        let mut groups = Vec::new();
        for d in v["blockdevices"].as_array().cloned().unwrap_or_default() {
            let t = st(&d, "type").unwrap_or_default();
            if (t != "disk" && t != "loop") || sz(&d) == 0 {
                continue;
            }
            let path = st(&d, "path").unwrap_or_default();
            let tran = st(&d, "tran").unwrap_or_default();
            let rm = bo(&d, "rm");
            let ssd = Some(!bo(&d, "rota"));
            let kind = if rm || tran == "usb" || t == "loop" { "external" } else { "internal" };
            let mut volumes = Vec::new();
            let mut mounts = Vec::new();
            for p in d["children"].as_array().cloned().unwrap_or_default() {
                let pp = st(&p, "path").unwrap_or_default();
                let fs = st(&p, "fstype");
                let m = st(&p, "mountpoint");
                if let Some(m) = &m {
                    mounts.push(m.clone());
                }
                let s = sz(&p);
                volumes.push(Drive {
                    id: pp.clone(),
                    label: st(&p, "label").unwrap_or_else(|| pp.clone()),
                    detail: detail_line(&[Some(human_size(s)), fs.clone(), m.clone()]),
                    device: pp,
                    size: s,
                    kind: kind.into(),
                    solid_state: ssd,
                    quick_supported: quick_supported(fs.as_deref()),
                    filesystem: fs,
                    mount_points: m.into_iter().collect(),
                    is_system: kind == "internal",
                    ..Default::default()
                });
            }
            let fs = st(&d, "fstype");
            if let Some(m) = st(&d, "mountpoint") {
                mounts.push(m);
            }
            let s = sz(&d);
            groups.push(DiskGroup {
                disk: Drive {
                    id: path.clone(),
                    label: st(&d, "model").unwrap_or_else(|| path.clone()),
                    detail: detail_line(&[Some(human_size(s)), Some(t.clone()), fs.clone()]),
                    device: path,
                    size: s,
                    kind: kind.into(),
                    solid_state: ssd,
                    quick_supported: volumes.is_empty() && quick_supported(fs.as_deref()),
                    filesystem: fs,
                    mount_points: mounts,
                    is_whole_disk: true,
                    is_system: kind == "internal",
                    warning: if kind == "internal" && ssd == Some(true) {
                        Some(TRIM_WARNING.into())
                    } else {
                        None
                    },
                    ..Default::default()
                },
                volumes,
            });
        }
        groups.sort_by_key(|g| g.disk.kind == "internal");
        Ok(groups)
    }
}
