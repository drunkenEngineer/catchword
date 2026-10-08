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
        "settings",
        "exclude_folder",
        "include_folder",
        "set_patterns",
        "finish_first_launch",
        "delete_all_data",
        "rebuild_index",
        "check_index",
        "notices",
        "set_appearance",
        "set_limits",
        "pause_indexing",
        "resume_indexing",
        "set_resource_mode",
        "set_pause_on_battery",
        "pick_index_folder",
        "move_index",
        "set_update_check",
        "check_for_update",
        "install_update",
        "set_detailed_logs",
        "diagnostics",
        "save_diagnostics",
        "preview",
        "open_file",
        "reveal_file",
    ]);
    // On Windows: run as the user, never elevated (SEC-4).
    let windows =
        tauri_build::WindowsAttributes::new().app_manifest(include_str!("windows-app.manifest"));
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(commands)
            .windows_attributes(windows),
    )
    .expect("could not prepare the Tauri build");
}
