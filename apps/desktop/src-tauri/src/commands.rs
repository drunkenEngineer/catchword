//! The commands the interface may call, and the state behind them.
//!
//! Every command is `async` and does its work on a blocking thread, so the
//! window never waits (PERF-2). Commands take ids, never paths (section 9).
//! The allow-list in build.rs and capabilities/main.json names each one;
//! the interface can call nothing else.

use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use catchword_engine::resolve_folder;
use catchword_service::{group_by_file, search as search_index, Model};
use catchword_store::Store;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::contract::{Folder, Meaning, SearchResponse, Status};
use crate::indexing::{Indexer, Notify};
use crate::settings::Settings;
use crate::{open, views};

/// The event that tells the interface to fetch the status again.
pub const STATUS_CHANGED: &str = "status-changed";

pub struct AppState {
    pub indexer: Indexer,
    settings: Mutex<Settings>,
    config_dir: PathBuf,
    /// A connection for searches, apart from the indexer's: reads never
    /// wait for writes (WAL).
    reader: Mutex<Store>,
    /// Said once, then cleared: why the settings were restored.
    notice: Mutex<Option<String>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    /// `data` is the app's folder in the non-roaming profile (PRIV-4).
    pub fn open(data: &Path) -> Result<Self> {
        let config_dir = data.join("config");
        let index = data.join("data").join("index.db");
        std::fs::create_dir_all(index.parent().unwrap_or(data))?;
        let (settings, notice) = Settings::load(&config_dir);
        let reader = Store::open(&index).context("cannot open the index")?;
        Ok(Self {
            indexer: Indexer::new(index),
            settings: Mutex::new(settings),
            config_dir,
            reader: Mutex::new(reader),
            notice: Mutex::new(notice),
        })
    }

    fn folders(&self) -> Vec<PathBuf> {
        lock(&self.settings)
            .folders
            .iter()
            .map(|folder| folder.path.clone())
            .collect()
    }

    pub fn start_indexing(&self, notify: Notify) {
        self.indexer.start(self.folders(), notify);
    }

    pub fn status(&self) -> Result<Status> {
        let snapshot = self.indexer.snapshot();
        let counts = lock(&self.reader).counts()?;
        let meaning = match self.indexer.model().as_deref() {
            None => Meaning::Loading,
            Some(Model::Ready(_)) => Meaning::Ready,
            Some(Model::Unavailable(reason)) => Meaning::Off {
                reason: reason.clone(),
            },
        };
        Ok(Status {
            folders: lock(&self.settings)
                .folders
                .iter()
                .map(|folder| Folder {
                    id: folder.id,
                    path: folder.path.to_string_lossy().to_string(),
                })
                .collect(),
            work: snapshot.work,
            files: counts.files,
            passages: counts.passages,
            searchable_by_meaning: counts.vectors,
            meaning,
            not_indexed: snapshot
                .not_indexed
                .iter()
                .map(|(path, reason)| views::not_indexed(path, *reason))
                .collect(),
            problem: lock(&self.notice).take().or(snapshot.problem),
        })
    }

    pub fn search(&self, query: &str) -> Result<SearchResponse> {
        let started = Instant::now();
        let loading = Model::Unavailable("the model is still loading".into());
        let model = self.indexer.model();
        let answer = search_index(
            &lock(&self.reader),
            model.as_deref().unwrap_or(&loading),
            query,
        )?;
        Ok(SearchResponse {
            notes: answer.notes.iter().map(|note| note.describe()).collect(),
            files: views::file_hits(group_by_file(answer.results)),
            elapsed_ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
        })
    }

    /// Add a folder the user chose in the native dialog, and index it.
    pub fn add_folder(&self, chosen: &Path, notify: Notify) -> Result<Option<Folder>> {
        let path = resolve_folder(chosen)
            .with_context(|| format!("cannot open folder {}", chosen.display()))?;
        let mut settings = lock(&self.settings);
        let Some(entry) = settings.add_folder(path) else {
            return Ok(None);
        };
        settings.save(&self.config_dir)?;
        drop(settings);
        self.start_indexing(notify);
        Ok(Some(Folder {
            id: entry.id,
            path: entry.path.to_string_lossy().to_string(),
        }))
    }

    /// Stop indexing, forget the folder and purge its text and vectors
    /// (IDX-6), then carry on with the other folders.
    pub fn remove_folder(&self, id: u32, notify: Notify) -> Result<()> {
        self.indexer.stop_and_wait();
        let mut settings = lock(&self.settings);
        let Some(entry) = settings.remove_folder(id) else {
            return Err(anyhow!("no folder with id {id}"));
        };
        settings.save(&self.config_dir)?;
        drop(settings);
        let mut prefix = entry.path.to_string_lossy().to_string();
        if !prefix.ends_with(MAIN_SEPARATOR) {
            prefix.push(MAIN_SEPARATOR);
        }
        lock(&self.reader).purge_missing(&prefix, &[])?;
        self.start_indexing(notify);
        Ok(())
    }

    pub fn preview(&self, id: i64) -> Result<Option<String>> {
        Ok(lock(&self.reader).passage_text(id)?)
    }

    /// The file of a passage, from the index.
    fn file(&self, id: i64) -> Result<PathBuf> {
        lock(&self.reader)
            .file_of_passage(id)?
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("this result is no longer in the index"))
    }
}

/// Run `work` with the app's state on a blocking thread.
async fn on_state<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&AppHandle, &AppState) -> Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        work(&app, &state).map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Tell the interface to fetch the status again.
pub fn notifier(app: &AppHandle) -> Notify {
    let app = app.clone();
    Arc::new(move || {
        let _ = app.emit(STATUS_CHANGED, ());
    })
}

#[tauri::command]
pub async fn status(app: AppHandle) -> Result<Status, String> {
    on_state(app, |_, state| state.status()).await
}

#[tauri::command]
pub async fn search(app: AppHandle, query: String) -> Result<SearchResponse, String> {
    on_state(app, move |_, state| state.search(&query)).await
}

/// Opens the native folder dialog here, in the shell: the interface never
/// sends a path (section 9, rule 7).
#[tauri::command]
pub async fn add_folder(app: AppHandle) -> Result<Option<Folder>, String> {
    on_state(app, |app, state| {
        let Some(chosen) = app
            .dialog()
            .file()
            .set_title("Choose a folder to search")
            .blocking_pick_folder()
        else {
            return Ok(None);
        };
        let chosen = chosen.into_path().map_err(|error| anyhow!("{error}"))?;
        state.add_folder(&chosen, notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn remove_folder(app: AppHandle, id: u32) -> Result<(), String> {
    on_state(app, move |app, state| {
        state.remove_folder(id, notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn index_now(app: AppHandle) -> Result<(), String> {
    on_state(app, |app, state| {
        state.start_indexing(notifier(app));
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn preview(app: AppHandle, id: i64) -> Result<Option<String>, String> {
    on_state(app, move |_, state| state.preview(id)).await
}

#[tauri::command]
pub async fn open_file(app: AppHandle, id: i64) -> Result<(), String> {
    on_state(app, move |_, state| Ok(open::open_file(&state.file(id)?)?)).await
}

#[tauri::command]
pub async fn reveal_file(app: AppHandle, id: i64) -> Result<(), String> {
    on_state(app, move |_, state| {
        Ok(open::reveal_file(&state.file(id)?)?)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn state(name: &str) -> (AppState, PathBuf) {
        let data = std::env::temp_dir().join(format!("catchword-app-test-{name}"));
        let _ = std::fs::remove_dir_all(&data);
        let docs = data.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("lease.txt"), "the notice period is three months").unwrap();
        std::fs::write(docs.join("invoice.txt"), "invoice INV-7 is due").unwrap();
        let state = AppState::open(&data).unwrap();
        state
            .indexer
            .set_model(Model::Unavailable("not needed".into()));
        (state, docs)
    }

    fn wait(state: &AppState) {
        let started = Instant::now();
        while state.indexer.is_running() {
            assert!(started.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_folder_is_added_indexed_searched_and_removed_by_id() {
        let (state, docs) = state("flow");
        let folder = state.add_folder(&docs, Arc::new(|| {})).unwrap().unwrap();
        assert!(state.add_folder(&docs, Arc::new(|| {})).unwrap().is_none());
        wait(&state);

        let status = state.status().unwrap();
        assert_eq!(status.files, 2);
        assert_eq!(status.folders.len(), 1);
        assert!(matches!(status.meaning, Meaning::Off { .. }));

        let found = state.search("notice period").unwrap();
        assert_eq!(found.files[0].name, "lease.txt");
        let id = found.files[0].passages[0].id;
        assert_eq!(
            state.preview(id).unwrap().as_deref(),
            Some("the notice period is three months")
        );
        assert!(state.file(id).unwrap().ends_with("lease.txt"));

        state.remove_folder(folder.id, Arc::new(|| {})).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().files, 0);
        assert!(state.search("notice").unwrap().files.is_empty());
        assert!(state.remove_folder(folder.id, Arc::new(|| {})).is_err());
    }

    #[test]
    fn folders_are_remembered_across_starts() {
        let (state, docs) = state("remember");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let data = docs.parent().unwrap().to_path_buf();
        drop(state);
        let again = AppState::open(&data).unwrap();
        assert_eq!(again.folders().len(), 1);
    }
}
