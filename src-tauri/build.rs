fn main() {
    // On Windows the whole app runs as administrator (raw disk access needs it),
    // so Windows shows one UAC prompt when the app opens.
    #[cfg(windows)]
    {
        let windows = tauri_build::WindowsAttributes::new()
            .app_manifest(include_str!("windows-app.manifest"));
        tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
            .expect("failed to run tauri-build");
    }
    #[cfg(not(windows))]
    tauri_build::build()
}
