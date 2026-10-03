fn main() {
    // The interface may call these commands and no others (SEC-1). Each
    // becomes a permission, granted in capabilities/main.json.
    let commands = tauri_build::AppManifest::new().commands(&[
        "status",
        "search",
        "add_folder",
        "remove_folder",
        "index_now",
        "retry_failed",
        "preview",
        "open_file",
        "reveal_file",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(commands))
        .expect("could not prepare the Tauri build");
}
