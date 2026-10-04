//! Indexing on a background thread, so the window never waits (PERF-2).
//!
//! One run indexes every folder by words, then embeds what is new. Asking
//! for a run while one is going schedules another right after it. Each file
//! and each batch of vectors is saved as it goes, so stopping loses nothing.
//!
//! Indexing can be paused, by the user or when free disk space runs low;
//! a paused indexer starts no run until it is resumed (IDX-5, RSC-3).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use catchword_engine::extract::Limits;
use catchword_engine::Exclusions;
use catchword_service::{
    embed_missing, index_folder, Cutter, Model, Report, Threads, Unreachable, Worker,
};
use catchword_store::Store;

use crate::contract::{PauseReason, ResourceMode, Stage, Work};
use crate::disk;
use crate::log::{Logger, Value};

/// How often progress is reported to the interface, at most.
const REPORT_EVERY: Duration = Duration::from_millis(250);
/// How often free disk space is checked during a run.
const DISK_CHECK_EVERY: Duration = Duration::from_secs(2);
/// The pace of indexing is measured over this much of the recent past.
const PACE_WINDOW: Duration = Duration::from_secs(30);
/// Below this much measured time, no pace is given: the first seconds of a
/// run, through unchanged files, would promise far too much.
const PACE_MINIMUM: Duration = Duration::from_secs(3);

/// How fast one stage goes, over the last half minute, for the estimate.
#[derive(Default)]
struct Pace {
    stage: Option<Stage>,
    /// (when, how many done), oldest first, about a second apart.
    samples: VecDeque<(Instant, u64)>,
}

impl Pace {
    /// Note progress; returns items a second, once there is enough to tell.
    fn record(&mut self, stage: Stage, done: u64, now: Instant) -> Option<f32> {
        if self.stage != Some(stage) {
            self.stage = Some(stage);
            self.samples.clear();
        }
        let last = self.samples.back().copied();
        if last.is_none_or(|(when, _)| now.duration_since(when) >= Duration::from_secs(1)) {
            self.samples.push_back((now, done));
        }
        while self
            .samples
            .front()
            .is_some_and(|(when, _)| now.duration_since(*when) > PACE_WINDOW)
            && self.samples.len() > 2
        {
            self.samples.pop_front();
        }
        let (since, from) = *self.samples.front()?;
        let span = now.duration_since(since);
        if span < PACE_MINIMUM || done <= from {
            return None;
        }
        Some((done - from) as f32 / span.as_secs_f32())
    }
}

/// The work as the interface sees it, with the time left at its pace.
fn work(stage: Stage, done: u64, total: u64, per_second: Option<f32>) -> Work {
    let seconds_left = per_second
        .filter(|pace| *pace > 0.0)
        .map(|pace| (total.saturating_sub(done) as f32 / pace).ceil() as u64);
    Work {
        stage,
        done,
        total,
        per_second,
        seconds_left,
    }
}

/// Called when there is news for the interface.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Control {
    running: bool,
    again: bool,
    folders: Vec<PathBuf>,
    exclusions: Exclusions,
    limits: Limits,
    paused: Option<PauseReason>,
}

#[derive(Default, Clone)]
pub struct Snapshot {
    pub work: Option<Work>,
    pub problem: Option<String>,
    /// The folder being read now.
    pub scanning: Option<PathBuf>,
    /// Folders that could not be reached on their last scan (SRC-5).
    pub offline: Vec<PathBuf>,
    pub paused: Option<PauseReason>,
}

struct Shared {
    store_path: PathBuf,
    worker: Worker,
    log: Arc<Logger>,
    /// None while the model loads. Replaced when the resource mode changes.
    model: Mutex<Option<Arc<Model>>>,
    model_ready: Condvar,
    control: Mutex<Control>,
    finished: Condvar,
    /// Stop the current run, for a moment (stop_and_wait).
    stop: AtomicBool,
    /// Stop the current run, until resumed (a pause).
    halt: AtomicBool,
    /// Pause below this much free disk space.
    min_free: AtomicU64,
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
    pub fn new(store_path: PathBuf, worker: Worker, log: Arc<Logger>) -> Self {
        Self {
            shared: Arc::new(Shared {
                store_path,
                worker,
                log,
                model: Mutex::new(None),
                model_ready: Condvar::new(),
                control: Mutex::new(Control::default()),
                finished: Condvar::new(),
                stop: AtomicBool::new(false),
                halt: AtomicBool::new(false),
                min_free: AtomicU64::new(disk::MIN_FREE_BYTES),
                snapshot: Mutex::new(Snapshot::default()),
            }),
        }
    }

    /// The model once it has loaded, or None while it is loading.
    pub fn model(&self) -> Option<Arc<Model>> {
        lock(&self.shared.model).clone()
    }

    /// Runs wait for this: passages must be cut with the model's tokenizer
    /// from the start, or the index would be rebuilt once it arrives. A new
    /// model replaces the old one; a search holding the old one finishes.
    pub fn set_model(&self, model: Model) {
        *lock(&self.shared.model) = Some(Arc::new(model));
        self.shared.model_ready.notify_all();
    }

    /// Pause: the current run stops after its file or batch, and no run
    /// starts until `resume`.
    pub fn pause(&self, reason: PauseReason) {
        pause(&self.shared, reason);
    }

    pub fn paused(&self) -> Option<PauseReason> {
        lock(&self.shared.control).paused
    }

    /// The limits the next run reads files within (SRC-7).
    pub fn set_limits(&self, limits: Limits) {
        lock(&self.shared.control).limits = limits;
    }

    /// Lift a pause. The next `start` runs.
    pub fn resume(&self) {
        let mut control = lock(&self.shared.control);
        control.paused = None;
        self.shared.halt.store(false, Ordering::SeqCst);
        lock(&self.shared.snapshot).paused = None;
    }

    #[cfg(test)]
    pub fn set_min_free_bytes(&self, bytes: u64) {
        self.shared.min_free.store(bytes, Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.shared.snapshot).clone()
    }

    pub fn is_running(&self) -> bool {
        lock(&self.shared.control).running
    }

    /// Index `folders`, leaving out `exclusions`, on a background thread.
    /// If a run is going, another follows it, with the latest of both.
    /// While paused, they are kept for when indexing resumes.
    pub fn start(&self, folders: Vec<PathBuf>, exclusions: Exclusions, notify: Notify) {
        let mut control = lock(&self.shared.control);
        control.folders = folders;
        control.exclusions = exclusions;
        if control.paused.is_some() {
            return;
        }
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
                let (folders, exclusions, limits) = {
                    let control = lock(&shared.control);
                    (
                        control.folders.clone(),
                        control.exclusions.clone(),
                        control.limits.clone(),
                    )
                };
                run(&shared, &folders, &exclusions, &limits, &notify);
                let mut control = lock(&shared.control);
                if control.again && !shared.stop.load(Ordering::SeqCst) && control.paused.is_none()
                {
                    control.again = false;
                    continue;
                }
                control.running = false;
                control.again = false;
                let mut snapshot = lock(&shared.snapshot);
                snapshot.work = None;
                snapshot.scanning = None;
                drop(snapshot);
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

fn pause(shared: &Shared, reason: PauseReason) {
    let mut control = lock(&shared.control);
    control.paused = Some(reason);
    shared.halt.store(true, Ordering::SeqCst);
    lock(&shared.snapshot).paused = Some(reason);
}

/// True if free space on the index's drive is below the minimum; then
/// indexing pauses (RSC-3). Where free space cannot be told, it is not low.
fn low_disk(shared: &Shared) -> bool {
    let Some(free) = disk::free_bytes(&shared.store_path) else {
        return false;
    };
    if free >= shared.min_free.load(Ordering::SeqCst) {
        return false;
    }
    shared
        .log
        .warn("index.low_disk", &[("free_mb", Value::Number(free >> 20))]);
    pause(shared, PauseReason::LowDisk);
    true
}

/// The threads embedding may use in a resource mode, all at below-normal
/// priority (RSC-2). The default mode uses at most half the logical cores
/// (section 14).
pub fn threads(mode: ResourceMode) -> Threads {
    let logical = thread::available_parallelism().map_or(2, |n| n.get());
    let count = match mode {
        ResourceMode::Light => 1,
        ResourceMode::Balanced => (logical / 2).max(1),
        ResourceMode::Fast => logical.saturating_sub(1).max(1),
    };
    Threads {
        count,
        background: Some(lower_priority),
    }
}

/// One run over all folders: words first, then meaning.
fn run(
    shared: &Shared,
    folders: &[PathBuf],
    exclusions: &Exclusions,
    limits: &Limits,
    notify: &Notify,
) {
    let model = {
        let mut model = lock(&shared.model);
        loop {
            if let Some(model) = model.as_ref() {
                break Arc::clone(model);
            }
            model = shared
                .model_ready
                .wait(model)
                .unwrap_or_else(PoisonError::into_inner);
        }
    };
    if low_disk(shared) {
        return;
    }
    let pace = Mutex::new(Pace::default());
    let set_work = |stage, done: u64, total: u64| {
        let per_second = lock(&pace).record(stage, done, Instant::now());
        lock(&shared.snapshot).work = Some(work(stage, done, total, per_second));
    };
    let log = &shared.log;
    let mut store = match Store::open(&shared.store_path) {
        Ok(store) => store,
        Err(error) => {
            log.error(
                "index.open_failed",
                &[("error", Value::Private(error.to_string()))],
            );
            lock(&shared.snapshot).problem = Some(format!("Cannot open the index: {error}"));
            return;
        }
    };
    log.info(
        "index.started",
        &[("folders", Value::Number(folders.len() as u64))],
    );
    let cutter = Cutter::for_model(&model);
    let mut last_report = Instant::now();
    let mut report = |force: bool| {
        if force || last_report.elapsed() >= REPORT_EVERY {
            last_report = Instant::now();
            notify();
        }
    };
    let mut last_disk_check = Instant::now();
    let mut keep_going = || {
        if last_disk_check.elapsed() >= DISK_CHECK_EVERY {
            last_disk_check = Instant::now();
            if low_disk(shared) {
                return false;
            }
        }
        !shared.stop.load(Ordering::SeqCst) && !shared.halt.load(Ordering::SeqCst)
    };

    let mut problem = None;
    for (number, folder) in folders.iter().enumerate() {
        let started = Instant::now();
        lock(&shared.snapshot).scanning = Some(folder.clone());
        let result = index_folder(
            &mut store,
            folder,
            exclusions,
            &cutter,
            &shared.worker,
            limits,
            |done, total| {
                set_work(Stage::Words, done as u64, total as u64);
                report(false);
                keep_going()
            },
        );
        lock(&shared.snapshot).scanning = None;
        match result {
            Ok(done) => {
                log_folder(log, number, &done, started);
                lock(&shared.snapshot)
                    .offline
                    .retain(|offline| offline != folder);
                if done.stopped {
                    return;
                }
            }
            // An unplugged drive or a moved folder: offline, not deleted.
            // Its files stay in the index (SRC-5), and Library says so.
            Err(error) if error.downcast_ref::<Unreachable>().is_some() => {
                log.warn(
                    "index.folder_offline",
                    &[
                        ("folder", Value::Number(number as u64 + 1)),
                        ("error", Value::Private(format!("{error:#}"))),
                    ],
                );
                let mut snapshot = lock(&shared.snapshot);
                if !snapshot.offline.contains(folder) {
                    snapshot.offline.push(folder.clone());
                }
            }
            Err(error) => {
                log.warn(
                    "index.folder_failed",
                    &[
                        ("folder", Value::Number(number as u64 + 1)),
                        ("error", Value::Private(format!("{error:#}"))),
                    ],
                );
                // A full disk shows as an error: then it is a pause.
                if low_disk(shared) {
                    return;
                }
                problem = Some(format!("{error:#}"));
            }
        }
        report(true);
    }
    // Every folder was gone through: a scan, even if one was offline.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64);
    if let Err(error) = store.set_last_scan(now) {
        log.warn(
            "index.last_scan_failed",
            &[("error", Value::Private(error.to_string()))],
        );
    }

    if let Some(ready) = model.ready() {
        let started = Instant::now();
        let before = store.counts().map_or(0, |counts| counts.vectors);
        let result = embed_missing(&mut store, ready, |done, total| {
            set_work(Stage::Meaning, done.max(0) as u64, total.max(0) as u64);
            report(false);
            keep_going()
        });
        match result {
            Ok(done) => {
                let after = store.counts().map_or(before, |counts| counts.vectors);
                log.info(
                    "embed.finished",
                    &[
                        (
                            "passages",
                            Value::Number(after.saturating_sub(before) as u64),
                        ),
                        ("millis", Value::Number(millis(started))),
                        ("model_changed", Value::Flag(done.model_changed)),
                        ("stopped", Value::Flag(done.stopped)),
                    ],
                );
            }
            Err(error) => {
                log.error(
                    "embed.failed",
                    &[("error", Value::Private(format!("{error:#}")))],
                );
                if low_disk(shared) {
                    return;
                }
                problem = Some(format!("{error:#}"));
            }
        }
    }
    lock(&shared.snapshot).problem = problem;
}

fn millis(since: Instant) -> u64 {
    since.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

/// What one folder's run did: counts always, each file's path only in
/// detailed logs.
fn log_folder(log: &Logger, number: usize, done: &Report, started: Instant) {
    let count = |n: usize| Value::Number(n as u64);
    log.info(
        "index.folder",
        &[
            ("folder", count(number + 1)),
            ("added", count(done.added)),
            ("reused", count(done.reused)),
            ("unchanged", count(done.unchanged)),
            ("removed", count(done.removed)),
            ("unsupported", count(done.unsupported)),
            ("skipped", count(done.skipped())),
            ("failed", count(done.failed())),
            ("known_problems", count(done.known_problems)),
            ("rebuilt", Value::Flag(done.rebuilt)),
            ("stopped", Value::Flag(done.stopped)),
            ("millis", Value::Number(millis(started))),
        ],
    );
    for (path, reason) in &done.not_indexed {
        log.debug(
            "index.not_indexed",
            &[
                ("reason", Value::Code(reason.code())),
                ("path", Value::Private(path.clone())),
            ],
        );
    }
}

/// Background indexing yields to the user's own work (RSC-2).
#[cfg(windows)]
pub fn lower_priority() {
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
pub fn lower_priority() {}

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
        let log = Arc::new(Logger::new(store.with_extension("logs"), false));
        let indexer = Indexer::new(store.clone(), Worker::NextToProgram, log);
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
    fn a_missing_folder_is_offline_and_the_others_are_indexed() {
        let (indexer, store) = indexer("missing");
        let gone = std::env::temp_dir().join("catchword-desktop-test-not-there");
        indexer.start(
            vec![gone.clone(), folder_with("missing-ok", 2)],
            Exclusions::default(),
            Arc::new(|| {}),
        );
        wait_until_idle(&indexer);
        assert_eq!(Store::open(&store).unwrap().counts().unwrap().files, 2);
        let snapshot = indexer.snapshot();
        assert_eq!(snapshot.offline, vec![gone]);
        assert_eq!(snapshot.scanning, None);
        // Offline is a state, not a problem.
        assert_eq!(snapshot.problem, None);
    }

    #[test]
    fn the_pace_is_measured_over_the_recent_past() {
        let start = Instant::now();
        let at = |secs: u64| start + Duration::from_secs(secs);
        let mut pace = Pace::default();
        // Too soon to tell.
        assert_eq!(pace.record(Stage::Words, 0, at(0)), None);
        assert_eq!(pace.record(Stage::Words, 50, at(2)), None);
        // 100 files in 4 seconds.
        assert_eq!(pace.record(Stage::Words, 100, at(4)), Some(25.0));
        // A fast start falls out of the window: 40 seconds on, the pace is
        // that of the last half minute.
        for secs in 5..=40 {
            pace.record(Stage::Words, 100 + (secs - 4) * 2, at(secs));
        }
        let recent = pace.record(Stage::Words, 174, at(41)).unwrap();
        assert!((recent - 2.0).abs() < 0.2, "{recent}");
        // A new stage starts afresh.
        assert_eq!(pace.record(Stage::Meaning, 10, at(42)), None);
    }

    #[test]
    fn the_time_left_follows_the_pace() {
        let busy = work(Stage::Meaning, 100, 1_100, Some(20.0));
        assert_eq!(busy.seconds_left, Some(50));
        assert_eq!(work(Stage::Words, 5, 10, None).seconds_left, None);
    }

    #[test]
    fn a_finished_run_records_the_time_of_the_scan() {
        let (indexer, store) = indexer("last-scan");
        indexer.start(
            vec![folder_with("last-scan", 2)],
            Exclusions::default(),
            Arc::new(|| {}),
        );
        wait_until_idle(&indexer);
        let last = Store::open(&store).unwrap().last_scan().unwrap().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!((now - last).abs() < 60);
    }

    /// A measurement, not a check: run it in a release build.
    /// `cargo test --release -p catchword-desktop measure -- --ignored --nocapture`
    #[test]
    #[ignore = "a measurement, with the real model"]
    fn measure_embedding_speed_in_each_resource_mode() {
        use catchword_embed::{Embedder, Paths, DEFAULT_MODEL};
        let vendor = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../vendor");
        let paths = Paths {
            runtime: vendor.join("onnxruntime/onnxruntime.dll"),
            model: vendor.join("models").join(DEFAULT_MODEL.name),
        };
        // Passages of about 300 words, as the cutter makes them.
        let words = [
            "lease", "notice", "period", "invoice", "payment", "contract", "tenant", "landlord",
            "months", "refund", "tax", "letter", "bank", "account", "the", "of", "and", "within",
            "days", "after",
        ];
        let passages: Vec<String> = (0..120)
            .map(|p| {
                (0..300)
                    .map(|w| words[(p * 7 + w * 13) % words.len()])
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        for mode in [
            ResourceMode::Light,
            ResourceMode::Balanced,
            ResourceMode::Fast,
        ] {
            let threads = threads(mode);
            let mut model = Embedder::load(&paths, &DEFAULT_MODEL, threads).unwrap();
            model.embed_passages(&[passages[0].as_str()]).unwrap();
            let started = Instant::now();
            for passage in &passages {
                model.embed_passages(&[passage.as_str()]).unwrap();
            }
            let per_second = passages.len() as f64 / started.elapsed().as_secs_f64();
            println!(
                "{mode:?}: {} threads, {per_second:.1} passages a second",
                threads.count
            );
        }
        // The default mode again, embedding several passages per call.
        let mut model =
            Embedder::load(&paths, &DEFAULT_MODEL, threads(ResourceMode::Balanced)).unwrap();
        for size in [1, 2, 4, 8, 16] {
            let started = Instant::now();
            for group in passages.chunks(size) {
                let group: Vec<&str> = group.iter().map(String::as_str).collect();
                model.embed_passages(&group).unwrap();
            }
            let per_second = passages.len() as f64 / started.elapsed().as_secs_f64();
            println!("Balanced, {size} per call: {per_second:.1} passages a second");
        }
    }

    #[test]
    fn a_pause_takes_effect_within_a_long_file() {
        let (indexer, store) = indexer("pause-soon");
        let folder = std::env::temp_dir().join("catchword-desktop-test-pause-soon");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        // Three million words: seconds of cutting, even on a fast computer.
        let words: Vec<String> = (0..3_000_000).map(|n| format!("w{}", n % 977)).collect();
        std::fs::write(folder.join("long.txt"), words.join(" ")).unwrap();

        indexer.start(vec![folder], Exclusions::default(), Arc::new(|| {}));
        thread::sleep(Duration::from_millis(400));
        let paused = Instant::now();
        indexer.pause(PauseReason::You);
        wait_until_idle(&indexer);
        // Seconds, not the whole file: generous, as other programs may be
        // busy; the proof is below.
        assert!(
            paused.elapsed() < Duration::from_secs(3),
            "{:?}",
            paused.elapsed()
        );
        // Stopped part way: nothing of the file is kept.
        assert_eq!(Store::open(&store).unwrap().counts().unwrap().files, 0);
    }

    #[test]
    fn a_paused_indexer_starts_nothing_until_resumed() {
        let (indexer, store) = indexer("paused");
        indexer.pause(PauseReason::You);
        indexer.start(
            vec![folder_with("paused", 3)],
            Exclusions::default(),
            Arc::new(|| {}),
        );
        wait_until_idle(&indexer);
        assert_eq!(indexer.snapshot().paused, Some(PauseReason::You));
        assert_eq!(Store::open(&store).unwrap().counts().unwrap().files, 0);

        indexer.resume();
        indexer.start(
            vec![folder_with("paused", 3)],
            Exclusions::default(),
            Arc::new(|| {}),
        );
        wait_until_idle(&indexer);
        assert_eq!(indexer.snapshot().paused, None);
        assert_eq!(Store::open(&store).unwrap().counts().unwrap().files, 3);
    }

    #[test]
    fn low_disk_space_pauses_indexing() {
        let (indexer, store) = indexer("low-disk");
        indexer.set_min_free_bytes(u64::MAX);
        indexer.start(
            vec![folder_with("low-disk", 3)],
            Exclusions::default(),
            Arc::new(|| {}),
        );
        wait_until_idle(&indexer);
        assert_eq!(indexer.snapshot().paused, Some(PauseReason::LowDisk));
        assert_eq!(Store::open(&store).unwrap().counts().unwrap().files, 0);
    }

    #[test]
    fn resource_modes_use_more_or_fewer_threads() {
        let logical = thread::available_parallelism().unwrap().get();
        assert_eq!(threads(ResourceMode::Light).count, 1);
        let balanced = threads(ResourceMode::Balanced).count;
        assert!(balanced >= 1 && balanced <= logical.div_ceil(2));
        assert!(threads(ResourceMode::Fast).count >= balanced);
        assert!(threads(ResourceMode::Fast).count < logical.max(2));
        assert!(threads(ResourceMode::Balanced).background.is_some());
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
