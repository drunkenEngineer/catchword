//! Indexing on a background thread, so the window never waits (PERF-2).
//!
//! One run indexes every folder by words, then embeds what is new. Asking
//! for a run while one is going schedules another right after it. Each file
//! and each batch of vectors is saved as it goes, so stopping loses nothing.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use catchword_engine::extract::Limits;
use catchword_engine::Exclusions;
use catchword_service::{embed_missing, index_folder, Cutter, Model, Worker};
use catchword_store::Store;

use crate::contract::{Stage, Work};

/// How often progress is reported to the interface, at most.
const REPORT_EVERY: Duration = Duration::from_millis(250);

/// Called when there is news for the interface.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Control {
    running: bool,
    again: bool,
    folders: Vec<PathBuf>,
    exclusions: Exclusions,
}

#[derive(Default, Clone)]
pub struct Snapshot {
    pub work: Option<Work>,
    pub problem: Option<String>,
}

struct Shared {
    store_path: PathBuf,
    worker: Worker,
    model: OnceLock<Arc<Model>>,
    control: Mutex<Control>,
    finished: Condvar,
    stop: AtomicBool,
    snapshot: Mutex<Snapshot>,
}

#[derive(Clone)]
pub struct Indexer {
    shared: Arc<Shared>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Indexer {
    /// `worker` is where the PDF reader is: beside the app, once installed.
    pub fn new(store_path: PathBuf, worker: Worker) -> Self {
        Self {
            shared: Arc::new(Shared {
                store_path,
                worker,
                model: OnceLock::new(),
                control: Mutex::new(Control::default()),
                finished: Condvar::new(),
                stop: AtomicBool::new(false),
                snapshot: Mutex::new(Snapshot::default()),
            }),
        }
    }

    /// The model once it has loaded, or None while it is loading.
    pub fn model(&self) -> Option<Arc<Model>> {
        self.shared.model.get().cloned()
    }

    /// Runs wait for this: passages must be cut with the model's tokenizer
    /// from the start, or the index would be rebuilt once it arrives.
    pub fn set_model(&self, model: Model) {
        let _ = self.shared.model.set(Arc::new(model));
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.shared.snapshot).clone()
    }

    pub fn is_running(&self) -> bool {
        lock(&self.shared.control).running
    }

    /// Index `folders`, leaving out `exclusions`, on a background thread.
    /// If a run is going, another follows it, with the latest of both.
    pub fn start(&self, folders: Vec<PathBuf>, exclusions: Exclusions, notify: Notify) {
        let mut control = lock(&self.shared.control);
        control.folders = folders;
        control.exclusions = exclusions;
        if control.running {
            control.again = true;
            return;
        }
        control.running = true;
        drop(control);
        let shared = Arc::clone(&self.shared);
        thread::spawn(move || {
            lower_priority();
            loop {
                let (folders, exclusions) = {
                    let control = lock(&shared.control);
                    (control.folders.clone(), control.exclusions.clone())
                };
                run(&shared, &folders, &exclusions, &notify);
                let mut control = lock(&shared.control);
                if control.again && !shared.stop.load(Ordering::SeqCst) {
                    control.again = false;
                    continue;
                }
                control.running = false;
                control.again = false;
                lock(&shared.snapshot).work = None;
                shared.finished.notify_all();
                break;
            }
            notify();
        });
    }

    /// Stop the current run, if any, and wait until it has ended. Needed
    /// before anything else writes to the index: it has one writer.
    pub fn stop_and_wait(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        let mut control = lock(&self.shared.control);
        while control.running {
            control = self
                .shared
                .finished
                .wait(control)
                .unwrap_or_else(PoisonError::into_inner);
        }
        self.shared.stop.store(false, Ordering::SeqCst);
    }
}

/// One run over all folders: words first, then meaning.
fn run(shared: &Shared, folders: &[PathBuf], exclusions: &Exclusions, notify: &Notify) {
    let model = Arc::clone(shared.model.wait());
    let set_work = |stage, done: u64, total: u64| {
        lock(&shared.snapshot).work = Some(Work { stage, done, total });
    };
    let mut store = match Store::open(&shared.store_path) {
        Ok(store) => store,
        Err(error) => {
            lock(&shared.snapshot).problem = Some(format!("Cannot open the index: {error}"));
            return;
        }
    };
    let cutter = Cutter::for_model(&model);
    let mut last_report = Instant::now();
    let mut report = |force: bool| {
        if force || last_report.elapsed() >= REPORT_EVERY {
            last_report = Instant::now();
            notify();
        }
    };

    let mut problem = None;
    for folder in folders {
        let result = index_folder(
            &mut store,
            folder,
            exclusions,
            &cutter,
            &shared.worker,
            &Limits::default(),
            |done, total| {
                set_work(Stage::Words, done as u64, total as u64);
                report(false);
                !shared.stop.load(Ordering::SeqCst)
            },
        );
        match result {
            Ok(done) if done.stopped => return,
            // Files not indexed are in the index's own list (COV-2).
            Ok(_) => {}
            // An unreachable folder (an unplugged drive) is skipped; its
            // files stay in the index (SRC-5).
            Err(error) => problem = Some(format!("{error:#}")),
        }
        report(true);
    }

    if let Some(ready) = model.ready() {
        let result = embed_missing(&mut store, ready, |done, total| {
            set_work(Stage::Meaning, done.max(0) as u64, total.max(0) as u64);
            report(false);
            !shared.stop.load(Ordering::SeqCst)
        });
        if let Err(error) = result {
            problem = Some(format!("{error:#}"));
        }
    }
    lock(&shared.snapshot).problem = problem;
}

/// Background indexing yields to the user's own work (RSC-2).
#[cfg(windows)]
fn lower_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
    };
    // SAFETY: GetCurrentThread returns a pseudo-handle that is always valid
    // for the calling thread; failure only leaves the priority unchanged.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

#[cfg(not(windows))]
fn lower_priority() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn folder_with(name: &str, files: usize) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("catchword-desktop-test-{name}"));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        for n in 0..files {
            std::fs::write(
                folder.join(format!("{n}.txt")),
                format!("document number {n}"),
            )
            .unwrap();
        }
        folder
    }

    fn indexer(name: &str) -> (Indexer, PathBuf) {
        let store = std::env::temp_dir().join(format!("catchword-desktop-test-{name}.db"));
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", store.display()));
        }
        let indexer = Indexer::new(store.clone(), Worker::NextToProgram);
        indexer.set_model(Model::Unavailable("not needed".into()));
        (indexer, store)
    }

    fn wait_until_idle(indexer: &Indexer) {
        let started = Instant::now();
        while indexer.is_running() {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "indexing never ended"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_run_indexes_every_folder_in_the_background_and_reports() {
        let (indexer, store) = indexer("run");
        let folders = vec![folder_with("run-a", 3), folder_with("run-b", 2)];
        let notified = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notified);
        indexer.start(
            folders,
            Exclusions::default(),
            Arc::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
            }),
        );
        wait_until_idle(&indexer);
        assert_eq!(Store::open(&store).unwrap().counts().unwrap().files, 5);
        assert!(notified.load(Ordering::SeqCst) >= 1);
        let snapshot = indexer.snapshot();
        assert!(snapshot.work.is_none());
        assert!(snapshot.problem.is_none());
    }

    #[test]
    fn a_missing_folder_is_a_problem_but_the_others_are_indexed() {
        let (indexer, store) = indexer("missing");
        let gone = std::env::temp_dir().join("catchword-desktop-test-not-there");
        indexer.start(
            vec![gone, folder_with("missing-ok", 2)],
            Exclusions::default(),
            Arc::new(|| {}),
        );
        wait_until_idle(&indexer);
        assert_eq!(Store::open(&store).unwrap().counts().unwrap().files, 2);
        assert!(indexer
            .snapshot()
            .problem
            .unwrap()
            .contains("cannot open folder"));
    }

    #[test]
    fn stop_and_wait_ends_a_run_and_leaves_the_index_usable() {
        let (indexer, store) = indexer("stop");
        indexer.start(
            vec![folder_with("stop", 200)],
            Exclusions::default(),
            Arc::new(|| {}),
        );
        indexer.stop_and_wait();
        assert!(!indexer.is_running());
        // Whatever was done is kept; the next run finishes the job.
        indexer.start(
            vec![folder_with("stop", 200)],
            Exclusions::default(),
            Arc::new(|| {}),
        );
        wait_until_idle(&indexer);
        assert_eq!(Store::open(&store).unwrap().counts().unwrap().files, 200);
    }
}
