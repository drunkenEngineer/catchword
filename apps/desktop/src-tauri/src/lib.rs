//! Catchword desktop app: the Tauri shell around the service.
//!
//! One window with three destinations (section 8). The interface in
//! apps/desktop/ui talks to this shell only through the commands in
//! `commands`, named in an allow-list, and shows document text as plain
//! text. The shell has no network code; the updater, when it comes, will be
//! the only exception (ADR-8).

pub mod commands;
pub mod contract;
mod indexing;
mod open;
mod settings;
mod views;

use std::thread;

use catchword_service::{Model, Worker};
use tauri::{Emitter, Manager};

use crate::commands::{notifier, AppState, STATUS_CHANGED};

pub fn run() {
    tauri::Builder::default()
        // First, so a second start only brings this window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_local_data_dir()?;
            app.manage(AppState::open(&data, Worker::NextToProgram)?);
            // The model takes a few seconds; the window is usable meanwhile.
            let handle = app.handle().clone();
            thread::spawn(move || {
                let state = handle.state::<AppState>();
                state.indexer.set_model(Model::load());
                let _ = handle.emit(STATUS_CHANGED, ());
                state.start_indexing(notifier(&handle));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::search,
            commands::add_folder,
            commands::remove_folder,
            commands::index_now,
            commands::retry_failed,
            commands::preview,
            commands::open_file,
            commands::reveal_file,
        ])
        .run(tauri::generate_context!())
        .expect("Catchword could not start");
}
