//! Catchword desktop app: the Tauri shell around the service.
//!
//! One window with three destinations (section 8). The interface in
//! apps/desktop/ui talks to this shell only through the commands in
//! `commands`, named in an allow-list, and shows document text as plain
//! text. The shell has no network code; the updater, when it comes, will be
//! the only exception (ADR-8).

pub mod commands;
pub mod contract;
mod diagnostics;
mod disk;
mod indexing;
mod log;
mod open;
mod power;
mod session;
mod settings;
mod views;

use std::thread;

use catchword_service::{Model, Worker};
use tauri::{Emitter, Manager};

use crate::commands::{notifier, AppState, STATUS_CHANGED};

pub fn run() {
    log::mark_start();
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
            let unclean = session::start(&data);
            let state = AppState::open(&data, Worker::NextToProgram)?;
            if unclean >= session::UNCLEAN_FOR_SAFE_MODE {
                state.enter_safe_mode(unclean);
            }
            log::record_panics(state.log());
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_theme(commands::window_theme(state.theme()));
            }
            app.manage(state);
            // On battery, indexing waits for mains power (IDX-9). The first
            // look comes before indexing starts.
            let handle = app.handle().clone();
            thread::spawn(move || loop {
                let state = handle.state::<AppState>();
                state.power_changed(power::on_battery(), notifier(&handle));
                thread::sleep(power::CHECK_EVERY);
            });
            // The model takes a few seconds; the window is usable meanwhile.
            let handle = app.handle().clone();
            thread::spawn(move || {
                let state = handle.state::<AppState>();
                state.set_model(Model::load(indexing::threads(state.resource_mode())));
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
            commands::settings,
            commands::exclude_folder,
            commands::include_folder,
            commands::set_patterns,
            commands::finish_first_launch,
            commands::delete_all_data,
            commands::rebuild_index,
            commands::check_index,
            commands::notices,
            commands::set_appearance,
            commands::set_limits,
            commands::pause_indexing,
            commands::resume_indexing,
            commands::set_resource_mode,
            commands::set_pause_on_battery,
            commands::set_detailed_logs,
            commands::diagnostics,
            commands::save_diagnostics,
            commands::preview,
            commands::open_file,
            commands::reveal_file,
        ])
        .build(tauri::generate_context!())
        .expect("Catchword could not start")
        .run(|app, event| {
            // A normal end: the next start is not counted as after a crash.
            if let tauri::RunEvent::Exit = event {
                if let Ok(data) = app.path().app_local_data_dir() {
                    session::clean_end(&data);
                }
            }
        });
}
