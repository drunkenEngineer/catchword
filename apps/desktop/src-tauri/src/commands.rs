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
use catchword_engine::exclude::DEFAULT_PATTERNS;
use catchword_engine::resolve_folder;
use catchword_service::{group_by_file, search as search_index, Model, Worker};
use catchword_store::Store;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::contract::{Folder, Meaning, SearchResponse, SettingsView, Status};
use crate::diagnostics::{self, Facts};
use crate::indexing::{Indexer, Notify};
use crate::log::{self, Logger, Value};
use crate::settings::{self, FolderEntry, Settings};
use crate::{open, views};

/// The event that tells the interface to fetch the status again.
pub const STATUS_CHANGED: &str = "status-changed";

pub struct AppState {
    pub indexer: Indexer,
    settings: Mutex<Settings>,
    config_dir: PathBuf,
    /// The index database file.
    index: PathBuf,
    /// A connection for searches, apart from the indexer's: reads never
    /// wait for writes (WAL).
    reader: Mutex<Store>,
    /// Said once, then cleared: why the settings were restored.
    notice: Mutex<Option<String>>,
    log: Arc<Logger>,
    /// The diagnostics report the user last read, which is what is saved.
    report: Mutex<Option<String>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    /// `data` is the app's folder in the non-roaming profile (PRIV-4).
    pub fn open(data: &Path, worker: Worker) -> Result<Self> {
        let config_dir = data.join("config");
        let index = data.join("data").join("index.db");
        std::fs::create_dir_all(index.parent().unwrap_or(data))?;
        let (settings, notice) = Settings::load(&config_dir);
        let log = Arc::new(Logger::new(data.join("logs"), settings.detailed_logs));
        log.info(
            "app.started",
            &[
                ("version", Value::Code(env!("CARGO_PKG_VERSION"))),
                ("detailed", Value::Flag(settings.detailed_logs)),
            ],
        );
        if notice.is_some() {
            log.warn("settings.damaged", &[]);
        }
        let reader = Store::open(&index).context("cannot open the index")?;
        Ok(Self {
            indexer: Indexer::new(index.clone(), worker, Arc::clone(&log)),
            settings: Mutex::new(settings),
            config_dir,
            index,
            reader: Mutex::new(reader),
            notice: Mutex::new(notice),
            log,
            report: Mutex::new(None),
        })
    }

    pub fn log(&self) -> Arc<Logger> {
        Arc::clone(&self.log)
    }

    /// The model has loaded, or failed to.
    pub fn set_model(&self, model: Model) {
        match &model {
            Model::Ready(_) => self.log.info("model.ready", &[]),
            Model::Unavailable(why) => self
                .log
                .warn("model.off", &[("reason", Value::Private(why.clone()))]),
        }
        self.indexer.set_model(model);
    }

    fn folders(&self) -> Vec<PathBuf> {
        lock(&self.settings)
            .folders
            .iter()
            .map(|folder| folder.path.clone())
            .collect()
    }

    pub fn start_indexing(&self, notify: Notify) {
        let exclusions = lock(&self.settings).exclusions();
        self.indexer.start(self.folders(), exclusions, notify);
    }

    /// Change the settings, save them, and index again so the change takes
    /// effect at once (UI-4). Returns what `change` returned.
    fn change_settings<T>(
        &self,
        notify: Notify,
        change: impl FnOnce(&mut Settings) -> Result<T>,
    ) -> Result<T> {
        let mut settings = lock(&self.settings);
        let mut changed = settings.clone();
        let result = change(&mut changed)?;
        changed.save(&self.config_dir)?;
        *settings = changed;
        drop(settings);
        self.start_indexing(notify);
        Ok(result)
    }

    fn index_bytes(&self) -> u64 {
        ["", "-wal"]
            .iter()
            .filter_map(|suffix| {
                std::fs::metadata(format!("{}{suffix}", self.index.display())).ok()
            })
            .map(|meta| meta.len())
            .sum()
    }

    pub fn settings(&self) -> Result<SettingsView> {
        let index_bytes = self.index_bytes();
        let settings = lock(&self.settings);
        Ok(SettingsView {
            excluded_folders: settings.excluded_folders.iter().map(folder_view).collect(),
            patterns: settings.patterns.clone(),
            default_patterns: DEFAULT_PATTERNS.iter().map(|p| p.to_string()).collect(),
            data_folder: self
                .index
                .parent()
                .unwrap_or(&self.index)
                .to_string_lossy()
                .to_string(),
            index_bytes,
            detailed_logs: settings.detailed_logs,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    pub fn set_detailed_logs(&self, on: bool) -> Result<()> {
        let mut settings = lock(&self.settings);
        settings.detailed_logs = on;
        settings.save(&self.config_dir)?;
        self.log.set_detailed(on);
        self.log.info("logs.detailed", &[("on", Value::Flag(on))]);
        Ok(())
    }

    /// Make the diagnostics report, and keep it: what the user reads is
    /// exactly what is saved.
    pub fn diagnostics(&self, include_paths: bool) -> Result<String> {
        let (meaning, meaning_detail) = match self.indexer.model().as_deref() {
            None => ("starting", None),
            Some(Model::Ready(_)) => ("ready", None),
            Some(Model::Unavailable(why)) => ("off", Some(why.clone())),
        };
        let (counts, model, problems) = {
            let reader = lock(&self.reader);
            (
                reader.counts()?,
                reader.embedding_model()?,
                reader.problems()?,
            )
        };
        let facts = Facts {
            made: std::time::SystemTime::now(),
            version: env!("CARGO_PKG_VERSION"),
            windows: diagnostics::windows_version(),
            meaning,
            meaning_detail,
            layout_version: catchword_store::SCHEMA_VERSION,
            counts,
            model,
            index_bytes: self.index_bytes(),
            problems,
            settings: lock(&self.settings).clone(),
            default_patterns: DEFAULT_PATTERNS.iter().map(|p| p.to_string()).collect(),
            detailed_logs: self.log.detailed(),
            log_lines: self.log.last_lines(diagnostics::LOG_LINES),
        };
        let text = diagnostics::report(&facts, include_paths);
        *lock(&self.report) = Some(text.clone());
        Ok(text)
    }

    /// The report last made, if any.
    fn prepared_report(&self) -> Option<String> {
        lock(&self.report).clone()
    }

    /// Leave out a folder the user chose in the native dialog. Its text
    /// leaves the index on the run that follows.
    pub fn exclude_folder(&self, chosen: &Path, notify: Notify) -> Result<Option<Folder>> {
        let path = resolve_folder(chosen)
            .with_context(|| format!("cannot open folder {}", chosen.display()))?;
        let entry = self.change_settings(notify, |settings| {
            settings.exclude_folder(path).map_err(|why| anyhow!(why))
        })?;
        Ok(entry.as_ref().map(folder_view))
    }

    pub fn include_folder(&self, id: u32, notify: Notify) -> Result<()> {
        self.change_settings(notify, |settings| {
            settings
                .include_folder(id)
                .map(drop)
                .ok_or_else(|| anyhow!("no excluded folder with id {id}"))
        })
    }

    pub fn set_patterns(&self, patterns: &[String], notify: Notify) -> Result<()> {
        self.change_settings(notify, |settings| {
            settings.set_patterns(patterns).map_err(|why| anyhow!(why))
        })
    }

    /// The first-launch steps are done; they are not shown again.
    pub fn finish_first_launch(&self) -> Result<()> {
        let mut settings = lock(&self.settings);
        settings.welcomed = true;
        settings.save(&self.config_dir)?;
        Ok(())
    }

    /// Delete the index and the settings, as if the app were new (APP-4).
    /// The user's own files are not touched.
    pub fn delete_all_data(&self) -> Result<()> {
        self.indexer.stop_and_wait();
        // The logs go too: detailed ones may name files.
        self.log.close();
        for name in log::FILES {
            remove_if_there(&self.log.folder().join(name))?;
        }
        self.log.set_detailed(false);
        *lock(&self.report) = None;
        // The settings first: with no folders left, nothing is indexed again.
        {
            let mut settings = lock(&self.settings);
            for name in settings::FILES {
                remove_if_there(&self.config_dir.join(name))?;
            }
            *settings = Settings::default();
        }
        let mut reader = lock(&self.reader);
        // Close the index, so its files can be deleted.
        *reader = Store::open_in_memory()?;
        for suffix in ["", "-wal", "-shm"] {
            remove_if_there(Path::new(&format!("{}{suffix}", self.index.display())))?;
        }
        *reader = Store::open(&self.index).context("cannot create a new index")?;
        self.log.info("data.deleted", &[]);
        Ok(())
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
        // One lock at a time: a guard lives to the end of its statement.
        let (folders, first_launch) = {
            let settings = lock(&self.settings);
            let folders = settings.folders.iter().map(folder_view).collect();
            (folders, !settings.welcomed)
        };
        let not_indexed = lock(&self.reader)
            .problems()?
            .iter()
            .map(views::not_indexed)
            .collect();
        let problem = lock(&self.notice).take().or(snapshot.problem);
        Ok(Status {
            folders,
            work: snapshot.work,
            files: counts.files,
            passages: counts.passages,
            searchable_by_meaning: counts.vectors,
            meaning,
            not_indexed,
            problem,
            first_launch,
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
        let response = SearchResponse {
            notes: answer.notes.iter().map(|note| note.describe()).collect(),
            files: views::file_hits(group_by_file(answer.results)),
            elapsed_ms: started.elapsed().as_millis().min(u32::MAX as u128) as u32,
        };
        // How long, and how much was found; never what was searched for.
        self.log.info(
            "search",
            &[
                ("millis", Value::Number(u64::from(response.elapsed_ms))),
                ("files", Value::Number(response.files.len() as u64)),
            ],
        );
        Ok(response)
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
        Ok(Some(folder_view(&entry)))
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

    /// Forget why the failed files were not indexed, so the next run reads
    /// them again, parked ones included (COV-2). Files skipped by a rule
    /// stay skipped: the same rule would skip them again.
    pub fn retry_failed(&self, notify: Notify) -> Result<()> {
        self.indexer.stop_and_wait();
        lock(&self.reader).forget_failures()?;
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

fn folder_view(entry: &FolderEntry) -> Folder {
    Folder {
        id: entry.id,
        path: entry.path.to_string_lossy().to_string(),
    }
}

fn remove_if_there(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Run `work` with the app's state on a blocking thread.
/// A failure is logged with the command's name; its details only in
/// detailed logs, as they may name a file.
async fn on_state<T: Send + 'static>(
    app: AppHandle,
    command: &'static str,
    work: impl FnOnce(&AppHandle, &AppState) -> Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        work(&app, &state).map_err(|error| {
            let message = format!("{error:#}");
            state.log.warn(
                "command.failed",
                &[
                    ("command", Value::Code(command)),
                    ("error", Value::Private(message.clone())),
                ],
            );
            message
        })
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
    on_state(app, "status", |_, state| state.status()).await
}

#[tauri::command]
pub async fn search(app: AppHandle, query: String) -> Result<SearchResponse, String> {
    on_state(app, "search", move |_, state| state.search(&query)).await
}

/// Opens the native folder dialog here, in the shell: the interface never
/// sends a path (section 9, rule 7).
#[tauri::command]
pub async fn add_folder(app: AppHandle) -> Result<Option<Folder>, String> {
    on_state(app, "add_folder", |app, state| {
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
    on_state(app, "remove_folder", move |app, state| {
        state.remove_folder(id, notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn index_now(app: AppHandle) -> Result<(), String> {
    on_state(app, "index_now", |app, state| {
        state.start_indexing(notifier(app));
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn retry_failed(app: AppHandle) -> Result<(), String> {
    on_state(app, "retry_failed", |app, state| {
        state.retry_failed(notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn settings(app: AppHandle) -> Result<SettingsView, String> {
    on_state(app, "settings", |_, state| state.settings()).await
}

/// Opens the native folder dialog here, as add_folder does.
#[tauri::command]
pub async fn exclude_folder(app: AppHandle) -> Result<Option<Folder>, String> {
    on_state(app, "exclude_folder", |app, state| {
        let Some(chosen) = app
            .dialog()
            .file()
            .set_title("Choose a folder to leave out")
            .blocking_pick_folder()
        else {
            return Ok(None);
        };
        let chosen = chosen.into_path().map_err(|error| anyhow!("{error}"))?;
        state.exclude_folder(&chosen, notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn include_folder(app: AppHandle, id: u32) -> Result<(), String> {
    on_state(app, "include_folder", move |app, state| {
        state.include_folder(id, notifier(app))
    })
    .await
}

/// Patterns are names, not paths: they are checked, and never opened.
#[tauri::command]
pub async fn set_patterns(app: AppHandle, patterns: Vec<String>) -> Result<(), String> {
    on_state(app, "set_patterns", move |app, state| {
        state.set_patterns(&patterns, notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn finish_first_launch(app: AppHandle) -> Result<(), String> {
    on_state(app, "finish_first_launch", |app, state| {
        state.finish_first_launch()?;
        notifier(app)();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_all_data(app: AppHandle) -> Result<(), String> {
    on_state(app, "delete_all_data", |app, state| {
        state.delete_all_data()?;
        notifier(app)();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn set_detailed_logs(app: AppHandle, on: bool) -> Result<(), String> {
    on_state(app, "set_detailed_logs", move |_, state| {
        state.set_detailed_logs(on)
    })
    .await
}

#[tauri::command]
pub async fn diagnostics(app: AppHandle, include_paths: bool) -> Result<String, String> {
    on_state(app, "diagnostics", move |_, state| {
        state.diagnostics(include_paths)
    })
    .await
}

/// Saves the report the user last read, where they choose in the native
/// dialog. Returns the file's name, or None if they cancelled.
#[tauri::command]
pub async fn save_diagnostics(app: AppHandle) -> Result<Option<String>, String> {
    on_state(app, "save_diagnostics", |app, state| {
        let text = state
            .prepared_report()
            .ok_or_else(|| anyhow!("Prepare the report first."))?;
        let Some(chosen) = app
            .dialog()
            .file()
            .set_title("Save the diagnostics report")
            .set_file_name("catchword-diagnostics.txt")
            .add_filter("Text", &["txt"])
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = chosen.into_path().map_err(|error| anyhow!("{error}"))?;
        std::fs::write(&path, text)?;
        Ok(path
            .file_name()
            .map(|name| name.to_string_lossy().to_string()))
    })
    .await
}

#[tauri::command]
pub async fn preview(app: AppHandle, id: i64) -> Result<Option<String>, String> {
    on_state(app, "preview", move |_, state| state.preview(id)).await
}

#[tauri::command]
pub async fn open_file(app: AppHandle, id: i64) -> Result<(), String> {
    on_state(app, "open_file", move |_, state| {
        Ok(open::open_file(&state.file(id)?)?)
    })
    .await
}

#[tauri::command]
pub async fn reveal_file(app: AppHandle, id: i64) -> Result<(), String> {
    on_state(app, "reveal_file", move |_, state| {
        Ok(open::reveal_file(&state.file(id)?)?)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The PDF reader as `cargo test --workspace` builds it.
    fn built_worker() -> Worker {
        let test = std::env::current_exe().unwrap();
        let folder = test.parent().and_then(Path::parent).unwrap();
        let worker = folder.join(format!("catchword-worker{}", std::env::consts::EXE_SUFFIX));
        assert!(
            worker.is_file(),
            "build the workspace first: cargo build --workspace"
        );
        Worker::At(worker)
    }

    fn state(name: &str) -> (AppState, PathBuf) {
        let data = std::env::temp_dir().join(format!("catchword-app-test-{name}"));
        let _ = std::fs::remove_dir_all(&data);
        let docs = data.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("lease.txt"), "the notice period is three months").unwrap();
        std::fs::write(docs.join("invoice.txt"), "invoice INV-7 is due").unwrap();
        let state = AppState::open(&data, built_worker()).unwrap();
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
    fn files_not_indexed_are_listed_and_failed_ones_can_be_retried() {
        let (state, docs) = state("retry");
        std::fs::write(docs.join("broken.pdf"), b"%PDF-1.7 and then nothing").unwrap();
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let listed = state.status().unwrap().not_indexed;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "broken.pdf");
        assert!(listed[0].failed && !listed[0].parked);

        // A second failure parks it; a retry reads it once more.
        state.start_indexing(Arc::new(|| {}));
        wait(&state);
        assert!(state.status().unwrap().not_indexed[0].parked);
        state.retry_failed(Arc::new(|| {})).unwrap();
        wait(&state);
        let listed = state.status().unwrap().not_indexed;
        assert!(listed[0].failed && !listed[0].parked);
    }

    #[test]
    fn an_excluded_folder_leaves_the_index_and_comes_back_when_included() {
        let (state, docs) = state("exclude");
        std::fs::create_dir(docs.join("private")).unwrap();
        std::fs::write(docs.join("private").join("diary.txt"), "the garden secret").unwrap();
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().files, 3);

        let excluded = state
            .exclude_folder(&docs.join("private"), Arc::new(|| {}))
            .unwrap()
            .unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().files, 2);
        assert!(state.search("garden secret").unwrap().files.is_empty());
        assert_eq!(
            state.settings().unwrap().excluded_folders,
            vec![excluded.clone()]
        );
        // A folder outside the chosen ones cannot be excluded.
        assert!(state
            .exclude_folder(&std::env::temp_dir(), Arc::new(|| {}))
            .is_err());

        state.include_folder(excluded.id, Arc::new(|| {})).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().files, 3);
    }

    #[test]
    fn a_pattern_leaves_out_matching_files_and_bad_patterns_are_refused() {
        let (state, docs) = state("patterns");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().files, 2);

        let mut patterns = state.settings().unwrap().patterns;
        patterns.push("invoice*".to_string());
        state.set_patterns(&patterns, Arc::new(|| {})).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().files, 1);

        let refused = state.set_patterns(&["C:/docs".to_string()], Arc::new(|| {}));
        assert!(refused.is_err());
        assert!(state
            .settings()
            .unwrap()
            .patterns
            .contains(&"invoice*".to_string()));
    }

    #[test]
    fn the_first_launch_is_shown_until_it_is_finished() {
        let (state, docs) = state("first-launch");
        assert!(state.status().unwrap().first_launch);
        state.finish_first_launch().unwrap();
        assert!(!state.status().unwrap().first_launch);
        let data = docs.parent().unwrap().to_path_buf();
        drop(state);
        let again = AppState::open(&data, built_worker()).unwrap();
        assert!(!again.status().unwrap().first_launch);
    }

    #[test]
    fn deleting_all_data_empties_the_index_and_forgets_the_settings() {
        let (state, docs) = state("delete-all");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        state.finish_first_launch().unwrap();
        wait(&state);
        assert!(state.settings().unwrap().index_bytes > 0);

        state.delete_all_data().unwrap();
        let status = state.status().unwrap();
        assert_eq!((status.files, status.folders.len()), (0, 0));
        assert!(status.first_launch);
        assert!(state.search("notice").unwrap().files.is_empty());
        // The user's files are untouched.
        assert!(docs.join("lease.txt").is_file());

        // And nothing comes back after a restart.
        let data = docs.parent().unwrap().to_path_buf();
        drop(state);
        let again = AppState::open(&data, built_worker()).unwrap();
        assert!(again.folders().is_empty());
        assert_eq!(again.status().unwrap().files, 0);
    }

    #[test]
    fn logs_never_hold_searches_document_text_or_paths() {
        let (state, docs) = state("logs-private");
        std::fs::write(docs.join("broken.pdf"), b"%PDF-1.7 and then nothing").unwrap();
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        state.search("notice period").unwrap();
        // A failing command is logged too.
        assert!(state.include_folder(999, Arc::new(|| {})).is_err());
        state.log.warn(
            "command.failed",
            &[("error", Value::Private(docs.display().to_string()))],
        );

        let logs = state.log.last_lines(1000).join("\n");
        for secret in [
            "notice",
            "three months",
            "invoice",
            "broken.pdf",
            "lease",
            "catchword-app-test",
        ] {
            assert!(!logs.contains(secret), "{secret} in:\n{logs}");
        }
        assert!(logs.contains(r#""event":"index.folder""#), "{logs}");
        assert!(logs.contains(r#""event":"search""#), "{logs}");
    }

    #[test]
    fn detailed_logs_name_files_and_the_report_leaves_them_out_unless_asked() {
        let (state, docs) = state("logs-detailed");
        std::fs::write(docs.join("broken.pdf"), b"%PDF-1.7 and then nothing").unwrap();
        state.set_detailed_logs(true).unwrap();
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        assert!(state.log.last_lines(1000).join("\n").contains("broken.pdf"));

        let report = state.diagnostics(false).unwrap();
        assert!(!report.contains("broken.pdf"), "{report}");
        assert!(!report.contains("catchword-app-test"), "{report}");
        assert!(report.contains("Files: 2"), "{report}");
        assert!(report.contains("Detailed logs: on"), "{report}");
        assert_eq!(state.prepared_report(), Some(report));

        let with_paths = state.diagnostics(true).unwrap();
        assert!(with_paths.contains("broken.pdf"), "{with_paths}");
        assert!(state.settings().unwrap().detailed_logs);
    }

    #[test]
    fn deleting_all_data_deletes_the_logs_too() {
        let (state, docs) = state("logs-deleted");
        state.set_detailed_logs(true).unwrap();
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        state.delete_all_data().unwrap();
        let logs = state.log.last_lines(1000);
        assert_eq!(logs.len(), 1, "{logs:?}");
        assert!(logs[0].contains("data.deleted"));
        assert!(!state.log.detailed());
    }

    #[test]
    fn folders_are_remembered_across_starts() {
        let (state, docs) = state("remember");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let data = docs.parent().unwrap().to_path_buf();
        drop(state);
        let again = AppState::open(&data, built_worker()).unwrap();
        assert_eq!(again.folders().len(), 1);
    }
}
