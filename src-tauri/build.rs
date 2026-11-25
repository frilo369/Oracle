fn main() {
    const COMMANDS: &[&str] = &["ready", "action", "guide_data"];
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS))
            .windows_attributes(
                tauri_build::WindowsAttributes::new_without_app_manifest()
                    .app_manifest(include_str!("oracle.manifest")),
            ),
    )
    .expect("Oracle metadata");
}
