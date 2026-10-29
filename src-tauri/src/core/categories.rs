//! File categories: which PhotoRec signatures to enable, and where each
//! recovered extension is sorted afterwards.

/// PhotoRec signature names per category. Every name here is verified to be
/// accepted by PhotoRec 7.1+ (an unknown name makes PhotoRec abort).
pub fn photorec_types(category: &str) -> &'static [&'static str] {
    match category {
        // "mov" covers mp4/mov/heic/cr3/3gp; "riff" covers webp; "tif" covers cr2/nef/arw/dng.
        "photos" => &[
            "jpg", "png", "gif", "tif", "bmp", "psd", "raf", "orf", "rw2", "crw", "x3f", "mrw",
            "ico", "xcf", "mov", "riff",
        ],
        "videos" => &[
            "mov", "riff", "mkv", "mpg", "flv", "asf", "m2ts", "ts", "swf", "wtv", "mxf", "r3d",
        ],
        "audio" => &["mp3", "flac", "ogg", "amr", "aif", "mid", "mov", "riff", "asf"],
        "documents" => &["pdf", "doc", "zip", "txt", "wpd", "one", "indd", "ps", "pst", "sqlite"],
        "archives" => &["zip", "7z", "rar", "gz", "bz2", "xz", "tar", "cab", "iso"],
        _ => &[],
    }
}

/// Build the PhotoRec /cmd file-option fragment for the chosen categories.
pub fn photorec_fileopt(categories: &[String]) -> String {
    if categories.is_empty() || categories.iter().any(|c| c == "everything") {
        // Everything except file-system structures (FAT tables, MFT records…),
        // which are not files anyone wants back. These names exist in PhotoRec 7.1–7.3.
        return "fileopt,everything,enable,fat,disable,mft,disable,hfsp,disable,xfs,disable".to_string();
    }
    let mut names: Vec<&str> = Vec::new();
    for c in categories {
        for n in photorec_types(c) {
            if !names.contains(n) {
                names.push(n);
            }
        }
    }
    let mut s = String::from("fileopt,everything,disable");
    for n in names {
        s.push(',');
        s.push_str(n);
        s.push_str(",enable");
    }
    s
}

pub const ORDER: &[(&str, &str)] = &[
    ("photos", "Photos"),
    ("videos", "Videos"),
    ("audio", "Audio"),
    ("documents", "Documents"),
    ("archives", "Archives"),
    ("other", "Other"),
];

pub fn folder_name(key: &str) -> &'static str {
    ORDER
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or("Other")
}

/// Category for a recovered file extension (lowercase, no dot).
pub fn category_of(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" | "jpe" | "png" | "gif" | "bmp" | "tif" | "tiff" | "heic" | "heif"
        | "webp" | "avif" | "psd" | "raf" | "orf" | "rw2" | "crw" | "cr2" | "cr3" | "nef"
        | "nrw" | "arw" | "srf" | "sr2" | "dng" | "x3f" | "mrw" | "pef" | "raw" | "rwl"
        | "3fr" | "erf" | "kdc" | "dcr" | "mos" | "iiq" | "ico" | "jp2" | "svg" | "xcf" => "photos",
        "mp4" | "mov" | "m4v" | "3gp" | "3g2" | "avi" | "mkv" | "webm" | "mpg" | "mpeg"
        | "m2ts" | "mts" | "ts" | "flv" | "wmv" | "asf" | "vob" | "mxf" | "r3d" | "wtv"
        | "swf" | "dv" => "videos",
        "mp3" | "m4a" | "m4b" | "aac" | "flac" | "ogg" | "opus" | "wav" | "aif" | "aiff"
        | "wma" | "amr" | "mid" | "midi" | "ape" | "caf" | "au" | "ra" => "audio",
        "pdf" | "doc" | "docx" | "dot" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods"
        | "odp" | "odg" | "rtf" | "txt" | "csv" | "html" | "htm" | "xml" | "json" | "md"
        | "pages" | "key" | "numbers" | "epub" | "one" | "wpd" | "ps" | "indd" | "tex"
        | "msg" | "eml" | "pst" | "mbox" | "vcf" | "ics" | "sqlite" | "db" => "documents",
        "zip" | "rar" | "7z" | "gz" | "tgz" | "bz2" | "xz" | "tar" | "cab" | "iso" | "dmg"
        | "apk" | "jar" => "archives",
        _ => "other",
    }
}

/// PhotoRec output that is file-system structure, not a user file.
pub fn is_structure(ext: &str) -> bool {
    matches!(ext, "fat" | "mft" | "hfsp" | "xfs" | "ext2")
}

/// Image formats that may carry EXIF "date taken".
pub fn has_exif(ext: &str) -> bool {
    matches!(
        ext,
        "jpg" | "jpeg" | "jpe" | "tif" | "tiff" | "heic" | "heif" | "png" | "webp" | "dng"
            | "cr2" | "nef" | "arw" | "orf" | "rw2" | "raf" | "pef"
    )
}

/// Images the webview can show as thumbnails.
pub fn previewable(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "heic" | "avif")
}

/// Quick Scan (The Sleuth Kit) can undelete with original names on these file systems.
/// ext2/3/4 is left out on purpose: deleting a file there wipes where its data lived,
/// so Quick Scan would only promise names it can't deliver. Deep Scan covers it.
pub fn quick_supported(fs: Option<&str>) -> bool {
    match fs.map(|s| s.to_ascii_lowercase()) {
        None => true, // unknown: let TSK try to detect
        Some(f) => {
            f.contains("fat")
                || f.contains("msdos")
                || f.contains("ntfs")
                || f.contains("exfat")
                || f.contains("ufs")
                || f.is_empty()
        }
    }
}
