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
use catchword_service::{group_by_file, search_within, search_words, Model, Worker};
use catchword_store::Store;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::contract::{
    FileAction, FileKind, Folder, FolderState, FolderStatus, IndexFolderChoice, Meaning,
    PauseReason, ResourceMode, SearchFilter, SearchResponse, SettingsView, Status, TextSize, Theme,
};
use crate::diagnostics::{self, Facts};
use crate::indexing::{self, Indexer, Notify};
use crate::log::{self, Logger, Value};
use crate::power;
use crate::settings::{self, FolderEntry, Settings};
use crate::{open, views};

/// The event that tells the interface to fetch the status again.
pub const STATUS_CHANGED: &str = "status-changed";

pub struct AppState {
    pub indexer: Indexer,
    settings: Mutex<Settings>,
    config_dir: PathBuf,
    /// The index database file, wherever it is now (APP-7).
    index: Mutex<PathBuf>,
    /// Its usual place, in the user's profile (PRIV-4).
    usual_index: PathBuf,
    /// The folder the user chose for the index, until they confirm the move.
    proposed: Mutex<Option<PathBuf>>,
    /// A connection for searches, apart from the indexer's: reads never
    /// wait for writes (WAL).
    reader: Mutex<Store>,
    /// Said once, then cleared: why the settings were restored.
    notice: Mutex<Option<String>>,
    log: Arc<Logger>,
    /// The interface has asked for the status: it is up (PERF-1).
    interface_up: std::sync::atomic::AtomicBool,
    /// The diagnostics report the user last read, which is what is saved.
    report: Mutex<Option<String>>,
    /// Whether the computer was on battery when last looked at (IDX-9).
    power: Mutex<Option<bool>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    /// `data` is the app's folder in the non-roaming profile (PRIV-4).
    pub fn open(data: &Path, worker: Worker) -> Result<Self> {
        let config_dir = data.join("config");
        let usual_index = data.join("data").join("index.db");
        std::fs::create_dir_all(usual_index.parent().unwrap_or(data))?;
        let (settings, notice) = Settings::load(&config_dir);
        let index = settings
            .index_folder
            .as_ref()
            .map_or_else(|| usual_index.clone(), |folder| folder.join("index.db"));
        // A folder the index was moved to, on a drive that is not connected:
        // wait for it, rather than start an empty index somewhere else.
        let away = settings
            .index_folder
            .as_ref()
            .filter(|folder| !folder.is_dir())
            .cloned();
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
        let (reader, opened) = match &away {
            Some(_) => (Store::open_in_memory()?, Opened::Away),
            None => open_index(&index, &log)?,
        };
        let indexer = Indexer::new(index.clone(), worker, Arc::clone(&log));
        if settings.paused {
            indexer.pause(PauseReason::You);
        }
        let notice = match opened {
            Opened::Fine => notice,
            Opened::Rebuilt => {
                let rebuilt = "The index was damaged, so it is being rebuilt from your files.";
                // The folders are kept in the settings, so they are intact
                // only if the settings are. Both can be damaged at once.
                Some(match notice {
                    Some(settings) => format!("{rebuilt} {settings}"),
                    None => format!("{rebuilt} Your folders and settings are intact."),
                })
            }
            Opened::Newer => {
                indexer.pause(PauseReason::NewerIndex);
                notice
            }
            Opened::Away => {
                indexer.pause(PauseReason::IndexAway);
                log.warn("index.away", &[]);
                let folder = away.as_deref().unwrap_or(&index).display();
                let away = format!(
                    "The index is kept in {folder}, which cannot be reached now: is its \
                     drive connected? Connect it, then resume indexing in Library, or move \
                     the index back to its usual place in Settings."
                );
                Some(match notice {
                    Some(settings) => format!("{away} {settings}"),
                    None => away,
                })
            }
        };
        Ok(Self {
            indexer,
            settings: Mutex::new(settings),
            config_dir,
            index: Mutex::new(index),
            usual_index,
            proposed: Mutex::new(None),
            reader: Mutex::new(reader),
            notice: Mutex::new(notice),
            log,
            interface_up: std::sync::atomic::AtomicBool::new(false),
            report: Mutex::new(None),
            power: Mutex::new(None),
        })
    }

    pub fn log(&self) -> Arc<Logger> {
        Arc::clone(&self.log)
    }

    /// Change how much of a file is read (SRC-7), then read again what the
    /// old limits skipped as too large. The current run is stopped first, so
    /// it cannot record them as too large again under the old limits.
    pub fn set_limits(&self, max_file_mb: u32, max_pages: u32, notify: Notify) -> Result<()> {
        self.indexer.stop_and_wait();
        {
            let mut settings = lock(&self.settings);
            let mut changed = settings.clone();
            changed
                .set_limits(max_file_mb, max_pages)
                .map_err(|why| anyhow!(why))?;
            changed.save(&self.config_dir)?;
            *settings = changed;
        }
        lock(&self.reader).forget(catchword_engine::extract::Reason::TooLarge)?;
        self.start_indexing(notify);
        Ok(())
    }

    /// Open in safe mode (spec section 19): indexing paused until the user
    /// resumes it, after `unclean` runs in a row ended without a clean exit.
    pub fn enter_safe_mode(&self, unclean: u32) {
        self.indexer.pause(PauseReason::SafeMode);
        self.log.warn(
            "start.safe_mode",
            &[("unclean_ends", Value::Number(u64::from(unclean)))],
        );
    }

    pub fn theme(&self) -> Theme {
        lock(&self.settings).theme
    }

    /// Save how the interface looks (APP-3). The interface applies it; the
    /// window's own frame follows the theme (see `window_theme`).
    pub fn set_appearance(&self, theme: Theme, text_size: TextSize) -> Result<()> {
        let mut settings = lock(&self.settings);
        settings.theme = theme;
        settings.text_size = text_size;
        settings.save(&self.config_dir)?;
        Ok(())
    }

    pub fn resource_mode(&self) -> ResourceMode {
        lock(&self.settings).resource_mode
    }

    /// Pause indexing until the user resumes it, even after a restart.
    pub fn pause_indexing(&self) -> Result<()> {
        let mut settings = lock(&self.settings);
        settings.paused = true;
        settings.save(&self.config_dir)?;
        drop(settings);
        self.indexer.pause(PauseReason::You);
        self.log.info("index.paused", &[]);
        Ok(())
    }

    /// Resume, whatever paused it. If disk space is still low, the run
    /// pauses again at once and says so.
    pub fn resume_indexing(&self, notify: Notify) -> Result<()> {
        if self.indexer.paused() == Some(PauseReason::IndexAway) && !self.reconnect_index()? {
            return Ok(());
        }
        let mut settings = lock(&self.settings);
        settings.paused = false;
        settings.save(&self.config_dir)?;
        drop(settings);
        self.indexer.resume();
        self.log.info("index.resumed", &[]);
        self.start_indexing(notify);
        Ok(())
    }

    /// The power source as it is now: on battery, indexing pauses; plugged
    /// in again, it carries on (IDX-9, see `power::step`).
    pub fn power_changed(&self, on_battery: Option<bool>, notify: Notify) {
        let enabled = lock(&self.settings).pause_on_battery;
        let before = std::mem::replace(&mut *lock(&self.power), on_battery);
        match power::step(before, on_battery, enabled, self.indexer.paused()) {
            power::Step::Pause => {
                self.indexer.pause(PauseReason::Battery);
                self.log.info("index.battery_pause", &[]);
                notify();
            }
            power::Step::Resume => {
                self.indexer.resume();
                self.log.info("index.battery_resume", &[]);
                self.start_indexing(Arc::clone(&notify));
                notify();
            }
            power::Step::Nothing => {}
        }
    }

    /// Whether indexing waits while on battery. Switched on, it takes effect
    /// at the next look even if already on battery; switched off, a battery
    /// pause ends there.
    pub fn set_pause_on_battery(&self, on: bool) -> Result<()> {
        let mut settings = lock(&self.settings);
        settings.pause_on_battery = on;
        settings.save(&self.config_dir)?;
        drop(settings);
        if on {
            *lock(&self.power) = None;
        }
        self.log
            .info("index.pause_on_battery", &[("on", Value::Flag(on))]);
        Ok(())
    }

    /// Save the resource mode. Returns true if the model must be loaded
    /// again for it to take effect (see `reload_model`).
    pub fn set_resource_mode(&self, mode: ResourceMode) -> Result<bool> {
        let mut settings = lock(&self.settings);
        let changed = settings.resource_mode != mode;
        settings.resource_mode = mode;
        settings.save(&self.config_dir)?;
        drop(settings);
        self.log
            .info("resources.mode", &[("mode", Value::Code(mode_code(mode)))]);
        Ok(changed && matches!(self.indexer.model().as_deref(), Some(Model::Ready(_))))
    }

    /// Load the model again with the current resource mode's threads, then
    /// carry on indexing. Searches use the old model meanwhile.
    pub fn reload_model(&self, notify: Notify) {
        self.indexer.stop_and_wait();
        self.set_model(Model::load(indexing::threads(self.resource_mode())));
        self.start_indexing(notify);
    }

    /// The model has loaded, or failed to.
    pub fn set_model(&self, model: Model) {
        match &model {
            Model::Ready(_) => self.log.info(
                "model.ready",
                &[("millis", Value::Number(log::since_start()))],
            ),
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
        let (exclusions, limits) = {
            let settings = lock(&self.settings);
            (settings.exclusions(), settings.limits())
        };
        self.indexer.set_limits(limits);
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
                std::fs::metadata(format!("{}{suffix}", self.index_path().display())).ok()
            })
            .map(|meta| meta.len())
            .sum()
    }

    pub fn settings(&self) -> Result<SettingsView> {
        let index_bytes = self.index_bytes();
        let index = self.index_path();
        let settings = lock(&self.settings);
        Ok(SettingsView {
            excluded_folders: settings.excluded_folders.iter().map(folder_view).collect(),
            patterns: settings.patterns.clone(),
            default_patterns: DEFAULT_PATTERNS.iter().map(|p| p.to_string()).collect(),
            data_folder: index
                .parent()
                .unwrap_or(&index)
                .to_string_lossy()
                .to_string(),
            index_bytes,
            detailed_logs: settings.detailed_logs,
            version: env!("CARGO_PKG_VERSION").to_string(),
            resource_mode: settings.resource_mode,
            data_synced_by: crate::disk::synced_by(&index).map(String::from),
            theme: settings.theme,
            text_size: settings.text_size,
            max_file_mb: settings.max_file_mb,
            max_pages: settings.max_pages,
            pause_on_battery: settings.pause_on_battery,
            index_moved: settings.index_folder.is_some(),
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
            crash_reports: log::crash_reports(self.log.folder())
                .iter()
                .filter_map(|report| std::fs::read_to_string(report).ok())
                .collect(),
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
        for report in log::crash_reports(self.log.folder()) {
            remove_if_there(&report)?;
        }
        let mut reader = lock(&self.reader);
        // Close the index, so its files can be deleted, wherever it is.
        *reader = Store::open_in_memory()?;
        let index = self.index_path();
        remove_index_files(&index)?;
        remove_if_there(&set_aside_path(&index))?;
        if index != self.usual_index {
            // Its own folder, now empty, goes too; a new index starts in
            // the usual place, as the settings now say.
            remove_own_folder(&index);
            self.set_index_path(self.usual_index.clone());
        }
        *reader = Store::open(&self.usual_index).context("cannot create a new index")?;
        drop(reader);
        if matches!(
            self.indexer.paused(),
            Some(PauseReason::NewerIndex | PauseReason::IndexAway)
        ) {
            self.indexer.resume();
        }
        self.log.info("data.deleted", &[]);
        Ok(())
    }

    pub fn status(&self) -> Result<Status> {
        // The interface's first question: the window is up and usable.
        if !self
            .interface_up
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            self.log.info(
                "interface.ready",
                &[("millis", Value::Number(log::since_start()))],
            );
        }
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
            let folders = settings
                .folders
                .iter()
                .map(|entry| FolderStatus {
                    id: entry.id,
                    path: entry.path.to_string_lossy().to_string(),
                    state: if snapshot.scanning.as_ref() == Some(&entry.path) {
                        FolderState::Scanning
                    } else if snapshot.offline.contains(&entry.path) {
                        FolderState::Offline
                    } else {
                        FolderState::Ready
                    },
                })
                .collect();
            (folders, !settings.welcomed)
        };
        let (not_indexed, last_scan_secs) = {
            let reader = lock(&self.reader);
            let problems = reader.problems()?.iter().map(views::not_indexed).collect();
            (problems, reader.last_scan()?)
        };
        // Said once; the interface keeps it until the user closes it.
        let notice = lock(&self.notice).take();
        Ok(Status {
            folders,
            work: snapshot.work,
            files: counts.files,
            passages: counts.passages,
            searchable_by_meaning: counts.vectors,
            meaning,
            not_indexed,
            problem: snapshot.problem,
            notice,
            first_launch,
            paused: snapshot.paused,
            last_scan_secs,
        })
    }

    pub fn search(&self, query: &str) -> Result<SearchResponse> {
        self.search_filtered(query, &SearchFilter::default(), false)
    }

    /// Search only a folder, or a kind of file, or both (SEA-6).
    /// `words_only`: by words and names alone, quickly, without the model;
    /// the interface shows these while the full search runs.
    pub fn search_filtered(
        &self,
        query: &str,
        filter: &SearchFilter,
        words_only: bool,
    ) -> Result<SearchResponse> {
        let started = Instant::now();
        let folders = match filter.folder {
            None => Vec::new(),
            Some(id) => {
                let settings = lock(&self.settings);
                let folder = settings
                    .folders
                    .iter()
                    .find(|folder| folder.id == id)
                    .ok_or_else(|| anyhow!("no folder with id {id}"))?;
                vec![folder.path.to_string_lossy().to_string()]
            }
        };
        let extensions: &[&str] = match filter.kind {
            None => &[],
            Some(FileKind::Pdf) => &["pdf"],
            Some(FileKind::Text) => &["txt", "md", "markdown"],
        };
        let filter = catchword_store::Filter {
            folders,
            extensions: extensions.iter().map(|e| e.to_string()).collect(),
        };
        let answer = if words_only {
            search_words(&lock(&self.reader), query, &filter)?
        } else {
            let loading = Model::Unavailable("the model is still loading".into());
            let model = self.indexer.model();
            search_within(
                &lock(&self.reader),
                model.as_deref().unwrap_or(&loading),
                query,
                &filter,
            )?
        };
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

    /// Delete the index and read every file again; folders and settings
    /// stay (IDX-7). Also the way out of an index from a newer version.
    pub fn rebuild_index(&self, notify: Notify) -> Result<()> {
        if self.indexer.paused() == Some(PauseReason::IndexAway) {
            return Err(anyhow!(
                "the index's folder cannot be reached: connect its drive, or move the index \
                 back to its usual place"
            ));
        }
        self.indexer.stop_and_wait();
        let index = self.index_path();
        let mut reader = lock(&self.reader);
        // Close the index, so its files can be deleted.
        *reader = Store::open_in_memory()?;
        remove_index_files(&index)?;
        remove_if_there(&set_aside_path(&index))?;
        *reader = Store::open(&index).context("cannot create a new index")?;
        drop(reader);
        if self.indexer.paused() == Some(PauseReason::NewerIndex) {
            self.indexer.resume();
        }
        self.log.info("index.rebuilt", &[]);
        self.start_indexing(notify);
        Ok(())
    }

    /// The index file, wherever it is now.
    fn index_path(&self) -> PathBuf {
        lock(&self.index).clone()
    }

    /// Point the app and the indexer at the index's new place.
    fn set_index_path(&self, index: PathBuf) {
        self.indexer.set_store_path(index.clone());
        *lock(&self.index) = index;
    }

    /// Note the folder the user chose for the index, inside which it would
    /// get a folder of its own, and say what the move would mean (APP-7).
    pub fn propose_index_folder(&self, chosen: &Path) -> IndexFolderChoice {
        let folder = chosen.join(INDEX_FOLDER);
        *lock(&self.proposed) = Some(folder.clone());
        IndexFolderChoice {
            path: folder.to_string_lossy().to_string(),
            synced_by: crate::disk::synced_by(&folder).map(String::from),
        }
    }

    /// Move the index to the folder last proposed, once the user confirmed.
    pub fn move_index_to_proposed(&self, notify: Notify) -> Result<()> {
        let folder = lock(&self.proposed)
            .take()
            .ok_or_else(|| anyhow!("choose a folder for the index first"))?;
        self.move_index(Some(folder), notify)
    }

    /// Move the index into `folder`, or back to its usual place for None
    /// (APP-7). The copy is checked before it is used, and the old one is
    /// deleted only then: it holds the text of the documents. Searches wait
    /// meanwhile. If anything fails, the index stays where it was.
    pub fn move_index(&self, folder: Option<PathBuf>, notify: Notify) -> Result<()> {
        let usual_folder = self.usual_index.parent().unwrap_or(&self.usual_index);
        let target = folder.clone().unwrap_or_else(|| usual_folder.to_path_buf());
        let to = target.join("index.db");
        let from = self.index_path();
        if to == from {
            return Err(anyhow!("the index is already there"));
        }
        if folder.is_some() && (to.exists() || set_aside_path(&to).exists()) {
            return Err(anyhow!(
                "{} already holds a Catchword index. Choose another folder, or delete that one first.",
                target.display()
            ));
        }
        let away = self.indexer.paused() == Some(PauseReason::IndexAway);
        self.indexer.stop_and_wait();
        let mut reader = lock(&self.reader);
        // Closed, everything in the index's log is in its file.
        *reader = Store::open_in_memory()?;
        let copied = if away {
            // Nothing to copy: a new index is made there and filled.
            std::fs::create_dir_all(&target)
                .with_context(|| format!("cannot make {}", target.display()))
        } else {
            copy_index(&from, &target)
        };
        if let Err(error) = copied {
            if !away {
                *reader = Store::open(&from).context("cannot open the index again")?;
            }
            drop(reader);
            self.start_indexing(notify);
            return Err(error);
        }
        {
            let mut settings = lock(&self.settings);
            settings.index_folder = folder.clone();
            settings.save(&self.config_dir)?;
        }
        self.set_index_path(to.clone());
        let (store, opened) = open_index(&to, &self.log)?;
        *reader = store;
        drop(reader);
        if !away {
            remove_index_files(&from)?;
            remove_if_there(&set_aside_path(&from))?;
            if from.parent() != Some(usual_folder) {
                remove_own_folder(&from);
            }
        }
        if away {
            self.indexer.resume();
        }
        if let Opened::Newer = opened {
            self.indexer.pause(PauseReason::NewerIndex);
        }
        self.log.info(
            "index.moved",
            &[("usual_place", Value::Flag(folder.is_none()))],
        );
        self.start_indexing(notify);
        Ok(())
    }

    /// The folder of an index that was away is back: open the index there.
    /// False if it was made by a newer version, so indexing stays paused.
    fn reconnect_index(&self) -> Result<bool> {
        let index = self.index_path();
        let folder = index.parent().unwrap_or(&index);
        if !folder.is_dir() {
            return Err(anyhow!(
                "{} still cannot be reached. Connect its drive, or move the index back to its usual place in Settings.",
                folder.display()
            ));
        }
        let (store, opened) = open_index(&index, &self.log)?;
        *lock(&self.reader) = store;
        self.log.info("index.back", &[]);
        if let Opened::Newer = opened {
            self.indexer.pause(PauseReason::NewerIndex);
            return Ok(false);
        }
        Ok(true)
    }

    /// The third-party notices shipped beside the program (REL-2), or None
    /// where there are none, as in a development build that was not packaged.
    pub fn notices(&self) -> Result<Option<String>> {
        Ok(read_first(&notices_places()))
    }

    /// The full integrity check, on demand: true if no damage was found.
    pub fn check_index(&self) -> Result<bool> {
        let sound = lock(&self.reader).integrity_check()?;
        self.log
            .info("index.checked", &[("sound", Value::Flag(sound))]);
        Ok(sound)
    }

    pub fn preview(&self, id: i64) -> Result<Option<String>> {
        Ok(lock(&self.reader).passage_text(id)?)
    }

    /// The file of a passage, if it is still where the index says; None
    /// if it was moved, renamed or deleted since the last scan.
    pub fn existing_file(&self, id: i64) -> Result<Option<PathBuf>> {
        let path = self.file(id)?;
        Ok(path.is_file().then_some(path))
    }

    /// The file of a passage, from the index.
    fn file(&self, id: i64) -> Result<PathBuf> {
        lock(&self.reader)
            .file_of_passage(id)?
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("this result is no longer in the index"))
    }
}

/// How the index was found at start.
enum Opened {
    Fine,
    /// It was damaged: set aside, and a fresh one made.
    Rebuilt,
    /// A newer version made it: untouched, and an empty one stands in.
    Newer,
    /// Its folder cannot be reached: an empty one stands in (APP-7).
    Away,
}

/// Open the index, after a quick check (section 12, corruption recovery).
/// A damaged one is set aside and a fresh one is made, which the scan at
/// start fills; the damaged one is deleted on the next good start, as it
/// holds private text. An index from a newer version is never written to.
fn open_index(index: &Path, log: &Logger) -> Result<(Store, Opened)> {
    match Store::open(index) {
        Ok(store) => {
            if store.quick_check().unwrap_or(false) {
                remove_if_there(&set_aside_path(index))?;
                return Ok((store, Opened::Fine));
            }
        }
        Err(error) if catchword_store::is_newer_layout(&error) => {
            log.warn("index.newer", &[]);
            return Ok((Store::open_in_memory()?, Opened::Newer));
        }
        Err(error) => log.error(
            "index.unreadable",
            &[("error", Value::Private(error.to_string()))],
        ),
    }
    log.error("index.damaged", &[]);
    let aside = set_aside_path(index);
    remove_if_there(&aside)?;
    std::fs::rename(index, &aside).context("cannot set the damaged index aside")?;
    remove_index_files(index)?;
    let store = Store::open(index).context("cannot create a new index")?;
    Ok((store, Opened::Rebuilt))
}

/// The notices file: beside the program, as installed; in a development
/// build also where scripts/notices.mjs writes it.
fn notices_places() -> Vec<PathBuf> {
    const NAME: &str = "THIRD-PARTY-NOTICES.txt";
    let mut places = Vec::new();
    if let Ok(program) = std::env::current_exe() {
        places.push(program.with_file_name(NAME));
    }
    if cfg!(debug_assertions) {
        places.push(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../target/notices")
                .join(NAME),
        );
    }
    places
}

/// The text of the first of `places` that holds a readable file.
fn read_first(places: &[PathBuf]) -> Option<String> {
    places
        .iter()
        .find_map(|place| std::fs::read_to_string(place).ok())
}

/// The folder of its own an index gets inside a folder the user chose, so
/// that nothing else there is ever touched (APP-7).
pub const INDEX_FOLDER: &str = "Catchword index";

/// Copy the closed index at `from` into `folder`, and check the copy before
/// it is put in place. A file left in the usual place before is Catchword's
/// own, so it is replaced.
fn copy_index(from: &Path, folder: &Path) -> Result<()> {
    std::fs::create_dir_all(folder).with_context(|| format!("cannot make {}", folder.display()))?;
    let to = folder.join("index.db");
    if !from.is_file() {
        // Nothing yet: a new index is made there.
        return Ok(());
    }
    let size = std::fs::metadata(from)?.len();
    if let Some(free) = crate::disk::free_bytes(folder) {
        if free < size + crate::disk::MIN_FREE_BYTES {
            return Err(anyhow!(
                "not enough free space in {}: the index needs {} MB, and 1 GB must stay free",
                folder.display(),
                size.div_ceil(1 << 20)
            ));
        }
    }
    let partial = folder.join("index.db.partial");
    let checked = (|| -> Result<()> {
        std::fs::copy(from, &partial).context("cannot copy the index")?;
        let sound = Store::open(&partial)
            .and_then(|copy| copy.quick_check())
            .unwrap_or(false);
        if !sound {
            return Err(anyhow!("the copy of the index could not be read back"));
        }
        remove_index_files(&to)?;
        std::fs::rename(&partial, &to).context("cannot put the copy in place")?;
        Ok(())
    })();
    if checked.is_err() {
        let _ = remove_index_files(&partial);
    }
    checked
}

/// Remove the folder of its own an index had, once empty. A folder holding
/// anything else is left as it is.
fn remove_own_folder(index: &Path) {
    if let Some(folder) = index.parent() {
        if folder.file_name() == Some(std::ffi::OsStr::new(INDEX_FOLDER)) {
            let _ = std::fs::remove_dir(folder);
        }
    }
}

/// Where a damaged index is kept until the next good start.
fn set_aside_path(index: &Path) -> PathBuf {
    index.with_file_name("index.damaged.db")
}

/// The index file and SQLite's two files beside it.
fn remove_index_files(index: &Path) -> std::io::Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        remove_if_there(Path::new(&format!("{}{suffix}", index.display())))?;
    }
    Ok(())
}

/// The window frame's theme: None follows Windows.
pub fn window_theme(theme: Theme) -> Option<tauri::Theme> {
    match theme {
        Theme::System => None,
        Theme::Light => Some(tauri::Theme::Light),
        Theme::Dark => Some(tauri::Theme::Dark),
    }
}

fn mode_code(mode: ResourceMode) -> &'static str {
    match mode {
        ResourceMode::Light => "light",
        ResourceMode::Balanced => "balanced",
        ResourceMode::Fast => "fast",
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
pub async fn search(
    app: AppHandle,
    query: String,
    filter: Option<SearchFilter>,
    words_only: Option<bool>,
) -> Result<SearchResponse, String> {
    on_state(app, "search", move |_, state| {
        state.search_filtered(
            &query,
            &filter.unwrap_or_default(),
            words_only.unwrap_or(false),
        )
    })
    .await
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
pub async fn pause_indexing(app: AppHandle) -> Result<(), String> {
    on_state(app, "pause_indexing", |app, state| {
        state.pause_indexing()?;
        notifier(app)();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn resume_indexing(app: AppHandle) -> Result<(), String> {
    on_state(app, "resume_indexing", |app, state| {
        state.resume_indexing(notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn set_resource_mode(app: AppHandle, mode: ResourceMode) -> Result<(), String> {
    on_state(app, "set_resource_mode", move |app, state| {
        if state.set_resource_mode(mode)? {
            // Loading the model takes a few seconds: not on this thread.
            let app = app.clone();
            std::thread::spawn(move || {
                let state = app.state::<AppState>();
                state.reload_model(notifier(&app));
            });
        }
        Ok(())
    })
    .await
}

/// Opens the native folder dialog here, in the shell; the move waits for
/// the user to confirm it (`move_index`).
#[tauri::command]
pub async fn pick_index_folder(app: AppHandle) -> Result<Option<IndexFolderChoice>, String> {
    on_state(app, "pick_index_folder", |app, state| {
        let Some(chosen) = app
            .dialog()
            .file()
            .set_title("Choose where to keep the index")
            .blocking_pick_folder()
        else {
            return Ok(None);
        };
        let chosen = chosen.into_path().map_err(|error| anyhow!("{error}"))?;
        Ok(Some(state.propose_index_folder(&chosen)))
    })
    .await
}

/// Move the index to the folder just picked, or back to its usual place.
#[tauri::command]
pub async fn move_index(app: AppHandle, to_usual_place: bool) -> Result<(), String> {
    on_state(app, "move_index", move |app, state| {
        if to_usual_place {
            state.move_index(None, notifier(app))
        } else {
            state.move_index_to_proposed(notifier(app))
        }
    })
    .await
}

#[tauri::command]
pub async fn set_pause_on_battery(app: AppHandle, on: bool) -> Result<(), String> {
    on_state(app, "set_pause_on_battery", move |app, state| {
        state.set_pause_on_battery(on)?;
        state.power_changed(crate::power::on_battery(), notifier(app));
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
pub async fn rebuild_index(app: AppHandle) -> Result<(), String> {
    on_state(app, "rebuild_index", |app, state| {
        state.rebuild_index(notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn set_limits(app: AppHandle, max_file_mb: u32, max_pages: u32) -> Result<(), String> {
    on_state(app, "set_limits", move |app, state| {
        state.set_limits(max_file_mb, max_pages, notifier(app))
    })
    .await
}

#[tauri::command]
pub async fn set_appearance(
    app: AppHandle,
    theme: Theme,
    text_size: TextSize,
) -> Result<(), String> {
    on_state(app, "set_appearance", move |app, state| {
        state.set_appearance(theme, text_size)?;
        if let Some(window) = app.get_webview_window("main") {
            window.set_theme(window_theme(theme))?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn notices(app: AppHandle) -> Result<Option<String>, String> {
    on_state(app, "notices", |_, state| state.notices()).await
}

#[tauri::command]
pub async fn check_index(app: AppHandle) -> Result<bool, String> {
    on_state(app, "check_index", |_, state| state.check_index()).await
}

#[tauri::command]
pub async fn preview(app: AppHandle, id: i64) -> Result<Option<String>, String> {
    on_state(app, "preview", move |_, state| state.preview(id)).await
}

#[tauri::command]
pub async fn open_file(app: AppHandle, id: i64) -> Result<FileAction, String> {
    on_state(app, "open_file", move |_, state| {
        let Some(path) = state.existing_file(id)? else {
            return Ok(FileAction::Missing);
        };
        open::open_file(&path)?;
        Ok(FileAction::Done)
    })
    .await
}

#[tauri::command]
pub async fn reveal_file(app: AppHandle, id: i64) -> Result<FileAction, String> {
    on_state(app, "reveal_file", move |_, state| {
        let Some(path) = state.existing_file(id)? else {
            return Ok(FileAction::Missing);
        };
        open::reveal_file(&path)?;
        Ok(FileAction::Done)
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
        assert_eq!(
            Path::new(&found.files[0].path),
            Path::new(&found.files[0].folder).join("lease.txt")
        );
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
    fn a_folder_that_cannot_be_reached_is_offline_and_keeps_its_files() {
        let (state, docs) = state("offline");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().folders[0].state, FolderState::Ready);

        let unplugged = docs.with_file_name("docs-unplugged");
        std::fs::rename(&docs, &unplugged).unwrap();
        state.start_indexing(Arc::new(|| {}));
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!(status.folders[0].state, FolderState::Offline);
        assert_eq!(status.files, 2);
        assert_eq!(status.problem, None);
        assert_eq!(state.search("notice period").unwrap().files.len(), 1);

        std::fs::rename(&unplugged, &docs).unwrap();
        state.start_indexing(Arc::new(|| {}));
        wait(&state);
        assert_eq!(state.status().unwrap().folders[0].state, FolderState::Ready);
    }

    #[test]
    fn a_pause_holds_until_resumed_even_across_a_restart() {
        let (state, docs) = state("pause");
        state.pause_indexing().unwrap();
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!((status.paused, status.files), (Some(PauseReason::You), 0));

        let data = docs.parent().unwrap().to_path_buf();
        drop(state);
        let again = AppState::open(&data, built_worker()).unwrap();
        again
            .indexer
            .set_model(Model::Unavailable("not needed".into()));
        assert_eq!(again.status().unwrap().paused, Some(PauseReason::You));
        again.resume_indexing(Arc::new(|| {})).unwrap();
        wait(&again);
        let status = again.status().unwrap();
        assert_eq!((status.paused, status.files), (None, 2));
    }

    // Free space is read on Windows only (disk.rs); 0.1 is for Windows.
    #[cfg(windows)]
    #[test]
    fn low_disk_space_pauses_until_there_is_room_again() {
        let (state, docs) = state("low-disk");
        state.indexer.set_min_free_bytes(u64::MAX);
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().paused, Some(PauseReason::LowDisk));

        // Still no room: resuming pauses again, with the reason.
        state.resume_indexing(Arc::new(|| {})).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().paused, Some(PauseReason::LowDisk));

        state.indexer.set_min_free_bytes(0);
        state.resume_indexing(Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!((status.paused, status.files), (None, 2));
    }

    #[test]
    fn the_resource_mode_is_saved() {
        let (state, docs) = state("resource-mode");
        assert_eq!(
            state.settings().unwrap().resource_mode,
            ResourceMode::Balanced
        );
        // No model is loaded here, so there is nothing to load again.
        assert!(!state.set_resource_mode(ResourceMode::Light).unwrap());
        let data = docs.parent().unwrap().to_path_buf();
        drop(state);
        let again = AppState::open(&data, built_worker()).unwrap();
        assert_eq!(again.resource_mode(), ResourceMode::Light);
    }

    /// The data folder of a test state, and its index file.
    fn index_of(docs: &Path) -> (PathBuf, PathBuf) {
        let data = docs.parent().unwrap().to_path_buf();
        let index = data.join("data").join("index.db");
        (data, index)
    }

    /// Move the index into a folder of its own inside `chosen` (APP-7).
    fn move_to(state: &AppState, chosen: &Path) {
        std::fs::create_dir_all(chosen).unwrap();
        state.propose_index_folder(chosen);
        state.move_index_to_proposed(Arc::new(|| {})).unwrap();
        wait(state);
    }

    fn reopen(data: &Path) -> AppState {
        let state = AppState::open(data, built_worker()).unwrap();
        state
            .indexer
            .set_model(Model::Unavailable("not needed".into()));
        state
    }

    #[test]
    fn the_index_moves_to_a_chosen_folder_and_back() {
        let (state, docs) = state("move-index");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let (data, usual) = index_of(&docs);
        let chosen = data.join("elsewhere");
        std::fs::create_dir_all(&chosen).unwrap();
        std::fs::write(chosen.join("mine.txt"), "the user's own file").unwrap();

        let choice = state.propose_index_folder(&chosen);
        assert!(choice.path.ends_with(INDEX_FOLDER));
        assert_eq!(choice.synced_by, None);
        move_to(&state, &chosen);
        assert!(chosen.join(INDEX_FOLDER).join("index.db").is_file());
        // The old copy holds the documents' text: it is gone.
        assert!(!usual.exists());
        assert_eq!(state.search("notice period").unwrap().files.len(), 1);
        let view = state.settings().unwrap();
        assert!(view.index_moved);
        assert!(view.data_folder.ends_with(INDEX_FOLDER));

        // The next start finds it there.
        drop(state);
        let state = reopen(&data);
        assert_eq!(state.status().unwrap().files, 2);

        // Back to the usual place: the folder of its own goes, nothing else.
        state.move_index(None, Arc::new(|| {})).unwrap();
        wait(&state);
        assert!(usual.is_file());
        assert!(!chosen.join(INDEX_FOLDER).exists());
        assert!(chosen.join("mine.txt").is_file());
        assert_eq!(state.status().unwrap().files, 2);
        assert!(!state.settings().unwrap().index_moved);
    }

    #[test]
    fn a_folder_that_already_holds_an_index_is_refused() {
        let (state, docs) = state("move-refused");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let (data, usual) = index_of(&docs);
        let other = data.join("taken").join(INDEX_FOLDER).join("index.db");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        std::fs::write(&other, "another install's index").unwrap();
        state.propose_index_folder(&data.join("taken"));
        assert!(state.move_index_to_proposed(Arc::new(|| {})).is_err());
        // Nothing changed, and the other index is untouched.
        assert!(usual.is_file());
        assert_eq!(
            std::fs::read_to_string(&other).unwrap(),
            "another install's index"
        );
        assert_eq!(state.search("notice period").unwrap().files.len(), 1);
        // Nor can it move to where it is, or without a folder chosen.
        assert!(state.move_index(None, Arc::new(|| {})).is_err());
        assert!(state.move_index_to_proposed(Arc::new(|| {})).is_err());
    }

    #[test]
    fn an_index_on_a_drive_that_is_not_connected_waits_for_it() {
        let (state, docs) = state("move-away");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let (data, usual) = index_of(&docs);
        let drive = data.join("drive");
        move_to(&state, &drive);
        drop(state);

        // As if its drive were unplugged.
        let unplugged = data.join("drive-unplugged");
        std::fs::rename(&drive, &unplugged).unwrap();
        let state = reopen(&data);
        let status = state.status().unwrap();
        assert_eq!(status.paused, Some(PauseReason::IndexAway));
        assert!(status.notice.unwrap().contains("cannot be reached"));
        assert!(state.resume_indexing(Arc::new(|| {})).is_err());
        assert!(state.rebuild_index(Arc::new(|| {})).is_err());
        // Nothing is indexed anywhere else meanwhile.
        state.start_indexing(Arc::new(|| {}));
        wait(&state);
        assert_eq!(state.status().unwrap().files, 0);
        assert!(!usual.exists());

        // Plugged in again: resuming finds it.
        std::fs::rename(&unplugged, &drive).unwrap();
        state.resume_indexing(Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!((status.paused, status.files), (None, 2));
    }

    #[test]
    fn an_index_that_is_away_can_come_back_to_its_usual_place() {
        let (state, docs) = state("move-away-back");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let (data, usual) = index_of(&docs);
        let drive = data.join("drive");
        move_to(&state, &drive);
        drop(state);
        std::fs::rename(&drive, data.join("drive-unplugged")).unwrap();

        let state = reopen(&data);
        // A new index in the usual place, filled from the files.
        state.move_index(None, Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!((status.paused, status.files), (None, 2));
        assert!(usual.is_file());
    }

    #[test]
    fn deleting_all_data_deletes_a_moved_index_too() {
        let (state, docs) = state("move-delete");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let (data, _) = index_of(&docs);
        let chosen = data.join("elsewhere");
        move_to(&state, &chosen);
        std::fs::write(chosen.join("mine.txt"), "the user's own file").unwrap();
        state.delete_all_data().unwrap();
        assert!(!chosen.join(INDEX_FOLDER).exists());
        assert!(chosen.join("mine.txt").is_file());
        assert!(!state.settings().unwrap().index_moved);
    }

    #[test]
    fn a_damaged_index_is_set_aside_and_rebuilt_with_settings_intact() {
        let (state, docs) = state("damaged");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let (data, index) = index_of(&docs);
        drop(state);
        remove_index_files(&index).unwrap();
        std::fs::write(&index, b"this is not a database, just damage").unwrap();

        let state = AppState::open(&data, built_worker()).unwrap();
        state
            .indexer
            .set_model(Model::Unavailable("not needed".into()));
        let status = state.status().unwrap();
        assert!(status.notice.unwrap().contains("rebuilt"));
        assert_eq!(status.problem, None);
        assert_eq!(status.folders.len(), 1);
        // Said once: the interface keeps it on screen.
        assert_eq!(state.status().unwrap().notice, None);
        assert!(set_aside_path(&index).is_file());
        state.start_indexing(Arc::new(|| {}));
        wait(&state);
        assert_eq!(state.status().unwrap().files, 2);

        // The next good start deletes the damaged copy: it holds private text.
        drop(state);
        let again = AppState::open(&data, built_worker()).unwrap();
        assert!(!set_aside_path(&index).exists());
        assert_eq!(again.status().unwrap().files, 2);
    }

    #[test]
    fn an_index_from_a_newer_version_is_left_alone_until_rebuilt() {
        let (state, docs) = state("newer");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let (data, index) = index_of(&docs);
        drop(state);
        {
            let store = Store::open(&index).unwrap();
            drop(store);
            let conn = catchword_store::rusqlite::Connection::open(&index).unwrap();
            conn.execute(
                "UPDATE meta SET value = '999' WHERE key = 'schema_version'",
                (),
            )
            .unwrap();
        }
        let before = std::fs::read(&index).unwrap();

        let state = AppState::open(&data, built_worker()).unwrap();
        state
            .indexer
            .set_model(Model::Unavailable("not needed".into()));
        state.start_indexing(Arc::new(|| {}));
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!(status.paused, Some(PauseReason::NewerIndex));
        assert_eq!(status.files, 0);
        assert_eq!(std::fs::read(&index).unwrap(), before);

        state.rebuild_index(Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!((status.paused, status.files), (None, 2));
    }

    #[test]
    fn rebuilding_reads_everything_again_and_keeps_the_folders() {
        let (state, docs) = state("rebuild");
        std::fs::write(docs.join("broken.pdf"), b"%PDF-1.7 and then nothing").unwrap();
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        assert!(state.check_index().unwrap());
        state.rebuild_index(Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!((status.files, status.folders.len()), (2, 1));
        // The failed file was tried afresh: one attempt, not parked.
        assert!(!status.not_indexed[0].parked);
        assert!(state.check_index().unwrap());
    }

    #[test]
    fn notices_are_read_from_the_first_place_that_has_them() {
        let folder = std::env::temp_dir().join("catchword-app-test-notices");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let beside = folder.join("beside-the-program.txt");
        let built = folder.join("built.txt");
        std::fs::write(&built, "react 19.3.0 (MIT)").unwrap();
        assert_eq!(
            read_first(&[beside.clone(), built.clone()]).as_deref(),
            Some("react 19.3.0 (MIT)")
        );
        std::fs::write(&beside, "the packaged notices").unwrap();
        assert_eq!(
            read_first(&[beside, built]).as_deref(),
            Some("the packaged notices")
        );
        assert_eq!(read_first(&[folder.join("missing.txt")]), None);
        // A development build looks where scripts/notices.mjs writes.
        assert!(notices_places()
            .iter()
            .any(|place| place.ends_with("target/notices/THIRD-PARTY-NOTICES.txt")));
    }

    #[test]
    fn raising_the_size_limit_reads_what_was_too_large() {
        let (state, docs) = state("limits");
        // Two megabytes of text, against a limit of one.
        let big: String = "word ".repeat(420_000);
        std::fs::write(docs.join("big.txt"), big).unwrap();
        state.set_limits(1, 5_000, Arc::new(|| {})).unwrap();
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!(status.files, 2);
        assert_eq!(status.not_indexed[0].name, "big.txt");

        state.set_limits(3, 5_000, Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!(status.files, 3);
        assert!(status.not_indexed.is_empty());
        assert_eq!(state.settings().unwrap().max_file_mb, 3);
        assert!(state.set_limits(0, 5_000, Arc::new(|| {})).is_err());
    }

    #[test]
    fn safe_mode_holds_indexing_until_resumed() {
        let (state, docs) = state("safe-mode");
        state.enter_safe_mode(2);
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!(
            (status.paused, status.files),
            (Some(PauseReason::SafeMode), 0)
        );
        assert!(state
            .log
            .last_lines(50)
            .join("\n")
            .contains(r#""event":"start.safe_mode""#));
        state.resume_indexing(Arc::new(|| {})).unwrap();
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!((status.paused, status.files), (None, 2));
    }

    #[test]
    fn the_appearance_is_saved() {
        let (state, docs) = state("appearance");
        let view = state.settings().unwrap();
        assert_eq!(
            (view.theme, view.text_size),
            (Theme::System, TextSize::Normal)
        );
        state.set_appearance(Theme::Dark, TextSize::Larger).unwrap();
        let data = docs.parent().unwrap().to_path_buf();
        drop(state);
        let again = AppState::open(&data, built_worker()).unwrap();
        let view = again.settings().unwrap();
        assert_eq!(
            (view.theme, view.text_size),
            (Theme::Dark, TextSize::Larger)
        );
        assert_eq!(window_theme(Theme::Dark), Some(tauri::Theme::Dark));
        assert_eq!(window_theme(Theme::System), None);
    }

    #[test]
    fn the_log_says_when_the_interface_first_asked_for_the_status() {
        let (state, _) = state("interface-ready");
        state.status().unwrap();
        state.status().unwrap();
        let ready: Vec<String> = state
            .log
            .last_lines(100)
            .into_iter()
            .filter(|line| line.contains(r#""event":"interface.ready""#))
            .collect();
        assert_eq!(ready.len(), 1, "{ready:?}");
        assert!(ready[0].contains(r#""millis":"#));
    }

    #[test]
    fn a_search_can_keep_to_one_folder_or_kind() {
        let (state, docs) = state("filter");
        let other = docs.with_file_name("other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(
            other.join("notice.md"),
            "another notice period, in markdown",
        )
        .unwrap();
        let first = state.add_folder(&docs, Arc::new(|| {})).unwrap().unwrap();
        state.add_folder(&other, Arc::new(|| {})).unwrap();
        wait(&state);
        let names = |filter: SearchFilter| -> Vec<String> {
            let mut names: Vec<String> = state
                .search_filtered("notice period", &filter, false)
                .unwrap()
                .files
                .into_iter()
                .map(|file| file.name)
                .collect();
            names.sort();
            names
        };
        assert_eq!(
            names(SearchFilter::default()),
            vec!["lease.txt", "notice.md"]
        );
        let in_first = SearchFilter {
            folder: Some(first.id),
            kind: None,
        };
        assert_eq!(names(in_first), vec!["lease.txt"]);
        let pdfs = SearchFilter {
            folder: None,
            kind: Some(FileKind::Pdf),
        };
        assert!(names(pdfs).is_empty());
        let unknown = SearchFilter {
            folder: Some(999),
            kind: None,
        };
        assert!(state.search_filtered("notice", &unknown, false).is_err());

        // By words alone: the same files, without asking the model, so
        // without its notes either.
        let words = state
            .search_filtered("notice period", &SearchFilter::default(), true)
            .unwrap();
        let mut found: Vec<String> = words.files.into_iter().map(|file| file.name).collect();
        found.sort();
        assert_eq!(found, vec!["lease.txt", "notice.md"]);
        assert!(words.notes.is_empty());
    }

    #[test]
    fn indexing_waits_on_battery_and_carries_on_when_plugged_in() {
        let (state, docs) = state("battery");
        let quiet: Notify = Arc::new(|| {});
        assert!(state.settings().unwrap().pause_on_battery);
        state.power_changed(Some(false), Arc::clone(&quiet));
        assert_eq!(state.status().unwrap().paused, None);

        state.power_changed(Some(true), Arc::clone(&quiet));
        assert_eq!(state.status().unwrap().paused, Some(PauseReason::Battery));
        // Nothing is indexed while it waits.
        state.add_folder(&docs, Arc::clone(&quiet)).unwrap();
        wait(&state);
        assert_eq!(state.status().unwrap().files, 0);

        // Plugged in: it carries on by itself.
        state.power_changed(Some(false), Arc::clone(&quiet));
        wait(&state);
        let status = state.status().unwrap();
        assert_eq!((status.paused, status.files), (None, 2));

        // The user's own pause is never lifted by the power source.
        state.pause_indexing().unwrap();
        state.power_changed(Some(true), Arc::clone(&quiet));
        state.power_changed(Some(false), Arc::clone(&quiet));
        assert_eq!(state.status().unwrap().paused, Some(PauseReason::You));
        state.resume_indexing(Arc::clone(&quiet)).unwrap();

        // Switched off, a battery pause ends; it is remembered.
        state.power_changed(Some(true), Arc::clone(&quiet));
        assert_eq!(state.status().unwrap().paused, Some(PauseReason::Battery));
        state.set_pause_on_battery(false).unwrap();
        state.power_changed(Some(true), Arc::clone(&quiet));
        assert_eq!(state.status().unwrap().paused, None);
        assert!(!state.settings().unwrap().pause_on_battery);
        // Switched on again while on battery, it pauses at the next look.
        state.set_pause_on_battery(true).unwrap();
        state.power_changed(Some(true), Arc::clone(&quiet));
        assert_eq!(state.status().unwrap().paused, Some(PauseReason::Battery));
        wait(&state);
    }

    #[test]
    fn a_result_whose_file_moved_is_reported_missing_not_opened() {
        let (state, docs) = state("missing-file");
        state.add_folder(&docs, Arc::new(|| {})).unwrap();
        wait(&state);
        let id = state.search("notice period").unwrap().files[0].passages[0].id;
        assert!(state.existing_file(id).unwrap().is_some());
        std::fs::rename(docs.join("lease.txt"), docs.join("lease-renamed.txt")).unwrap();
        assert_eq!(state.existing_file(id).unwrap(), None);
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
