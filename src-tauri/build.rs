fn main() {
    let mut windows = tauri_build::WindowsAttributes::new();
    let manifest = std::fs::read_to_string("app.manifest")
        .expect("Failed to read app.manifest");
    
    windows = windows.app_manifest(manifest);

    tauri_build::try_build(
        tauri_build::Attributes::new().windows_attributes(windows)
    ).expect("failed to run build script");
}