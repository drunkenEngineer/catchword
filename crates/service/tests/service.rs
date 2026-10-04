//! Indexing and search through the service, as both front ends use them.
//! The meaning-stage tests need `sh scripts/fetch-embedding.sh`.

use std::fs;
use std::path::Path;

use catchword_engine::extract::{Limits, Reason};
use catchword_engine::Exclusions;
use catchword_service::{
    embed_missing, group_by_file, index_folder, is_parked, search, Cutter, Model, Note, Report,
    Unreachable, Worker,
};
use catchword_store::Store;
use catchword_test_support::scratch_folder;

/// Three text files; no PDF, so no worker program is needed.
fn three_files(name: &str) -> std::path::PathBuf {
    let folder = scratch_folder(name);
    for (file, text) in [
        ("a.txt", "the lease notice period is three months"),
        ("b.txt", "the invoice is due in thirty days"),
        ("c.txt", "the cat sleeps on the windowsill"),
    ] {
        fs::write(folder.join(file), text).unwrap();
    }
    folder
}

fn index(store: &mut Store, folder: &Path, stop_after: usize) -> Report {
    let cutter = Cutter::for_model(&Model::Unavailable("not needed".into()));
    index_folder(
        store,
        folder,
        &Exclusions::default(),
        &cutter,
        &Worker::NextToProgram,
        &Limits::default(),
        |done, _| done < stop_after,
    )
    .unwrap()
}

#[test]
fn a_stopped_run_keeps_what_it_did_and_the_next_run_carries_on() {
    let folder = three_files("service-stop");
    let mut store = Store::open_in_memory().unwrap();

    let first = index(&mut store, &folder, 1);
    assert!(first.stopped);
    assert_eq!(first.added, 1);
    assert_eq!(store.counts().unwrap().files, 1);

    let second = index(&mut store, &folder, usize::MAX);
    assert!(!second.stopped);
    assert_eq!((second.unchanged, second.added), (1, 2));
    assert_eq!(store.counts().unwrap().files, 3);
}

#[test]
fn a_stopped_run_never_forgets_files_it_did_not_reach() {
    let folder = three_files("service-no-forget");
    let mut store = Store::open_in_memory().unwrap();
    index(&mut store, &folder, usize::MAX);

    // Stopped after one file: the other two were not seen, but are kept.
    let stopped = index(&mut store, &folder, 1);
    assert!(stopped.stopped);
    assert_eq!(stopped.removed, 0);
    assert_eq!(store.counts().unwrap().files, 3);

    // A full run does forget a file that is really gone.
    fs::remove_file(folder.join("c.txt")).unwrap();
    let full = index(&mut store, &folder, usize::MAX);
    assert_eq!(full.removed, 1);
}

#[test]
fn a_file_is_found_by_its_name_and_shows_its_date() {
    let folder = three_files("service-names");
    fs::create_dir(folder.join("Plumber")).unwrap();
    fs::write(
        folder.join("Plumber").join("quote.txt"),
        "forty pounds an hour",
    )
    .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    index(&mut store, &folder, usize::MAX);
    let answer = search(&store, &Model::Unavailable("off".into()), "plumber").unwrap();
    let files = group_by_file(answer.results);
    assert_eq!(files.len(), 1);
    assert!(files[0].path.ends_with("quote.txt"));
    assert!(files[0].modified_secs > 1_600_000_000);
}

#[test]
fn results_by_words_are_grouped_by_file() {
    let folder = three_files("service-search");
    let mut store = Store::open_in_memory().unwrap();
    index(&mut store, &folder, usize::MAX);
    let answer = search(&store, &Model::Unavailable("off".into()), "invoice due").unwrap();
    let files = group_by_file(answer.results);
    assert_eq!(files.len(), 1);
    assert!(files[0].path.ends_with("b.txt"));
}

#[test]
fn the_meaning_stage_can_stop_and_carry_on() {
    let model = Model::load(catchword_service::Threads::RUNTIME_DEFAULT);
    let Some(ready) = model.ready() else {
        panic!("no model; run sh scripts/fetch-embedding.sh");
    };
    let folder = three_files("service-meaning");
    let mut store = Store::open_in_memory().unwrap();
    let cutter = Cutter::for_model(&model);
    index_folder(
        &mut store,
        &folder,
        &Exclusions::default(),
        &cutter,
        &Worker::NextToProgram,
        &Limits::default(),
        |_, _| true,
    )
    .unwrap();

    let stopped = embed_missing(&mut store, ready, |_, _| false).unwrap();
    assert!(stopped.stopped);
    assert_eq!(store.counts().unwrap().vectors, 0);

    let finished = embed_missing(&mut store, ready, |_, _| true).unwrap();
    assert!(!finished.stopped);
    let counts = store.counts().unwrap();
    assert_eq!(counts.vectors, counts.passages);

    // Meaning finds the lease from a question that shares no keyword with it.
    let answer = search(&store, &model, "how long before I can move out").unwrap();
    assert!(answer
        .notes
        .iter()
        .all(|note| !matches!(note, Note::Partial { .. })));
    let first = group_by_file(answer.results).remove(0);
    assert!(first.path.ends_with("a.txt"), "{}", first.path);
}

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

fn index_with(store: &mut Store, folder: &Path, worker: &Worker, limits: &Limits) -> Report {
    let cutter = Cutter::for_model(&Model::Unavailable("not needed".into()));
    index_folder(
        store,
        folder,
        &Exclusions::default(),
        &cutter,
        worker,
        limits,
        |_, _| true,
    )
    .unwrap()
}

#[test]
fn a_skipped_file_is_not_read_again_until_it_changes() {
    let folder = scratch_folder("service-skip-remembered");
    fs::write(folder.join("big.txt"), "far more than ten bytes of text").unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let small = Limits {
        max_file_bytes: 10,
        ..Limits::default()
    };

    let first = index_with(&mut store, &folder, &Worker::NextToProgram, &small);
    assert_eq!(first.not_indexed.len(), 1);
    assert_eq!(first.not_indexed[0].1, Reason::TooLarge);
    assert_eq!(first.known_problems, 0);

    // Known now: still listed, but not read again.
    let second = index_with(&mut store, &folder, &Worker::NextToProgram, &small);
    assert_eq!(second.not_indexed.len(), 1);
    assert_eq!(second.known_problems, 1);
    assert_eq!(store.problems().unwrap()[0].attempts, 1);

    // A changed file gets a fresh try; once indexed, it is no problem.
    fs::write(folder.join("big.txt"), "now small").unwrap();
    let third = index_with(&mut store, &folder, &Worker::NextToProgram, &small);
    assert_eq!((third.added, third.not_indexed.len()), (1, 0));
    assert!(store.problems().unwrap().is_empty());
}

#[test]
fn a_file_that_fails_twice_is_parked_until_a_retry() {
    let folder = scratch_folder("service-parked");
    fs::write(
        folder.join("broken.pdf"),
        b"%PDF-1.7 and then nothing useful",
    )
    .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let worker = built_worker();
    let limits = Limits::default();
    let attempts = |store: &Store| store.problems().unwrap()[0].attempts;

    // Damaged, or the PDF library missing: either way, reading failed.
    let first = index_with(&mut store, &folder, &worker, &limits);
    assert!(first.not_indexed[0].1.is_failure(), "{first:?}");
    assert_eq!(attempts(&store), 1);

    let second = index_with(&mut store, &folder, &worker, &limits);
    assert_eq!((second.known_problems, attempts(&store)), (0, 2));
    assert!(is_parked(&store.problems().unwrap()[0]));

    // Parked: listed, not read again.
    let third = index_with(&mut store, &folder, &worker, &limits);
    assert_eq!((third.known_problems, attempts(&store)), (1, 2));
    assert_eq!(third.not_indexed.len(), 1);

    // The user's retry: tried again, counting from one.
    assert_eq!(store.forget_failures().unwrap(), 1);
    let fourth = index_with(&mut store, &folder, &worker, &limits);
    assert_eq!((fourth.known_problems, attempts(&store)), (0, 1));
}

#[test]
fn excluding_a_folder_removes_its_text_and_including_it_brings_it_back() {
    let folder = three_files("service-exclude");
    let private = folder.join("private");
    fs::create_dir(&private).unwrap();
    fs::write(private.join("diary.txt"), "a secret about the garden").unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let cutter = Cutter::for_model(&Model::Unavailable("not needed".into()));
    let mut run = |exclusions: &Exclusions| {
        index_folder(
            &mut store,
            &folder,
            exclusions,
            &cutter,
            &Worker::NextToProgram,
            &Limits::default(),
            |_, _| true,
        )
        .unwrap()
    };

    assert_eq!(run(&Exclusions::default()).added, 4);
    let excluded = Exclusions {
        folders: vec![catchword_engine::resolve_folder(&private).unwrap()],
        patterns: Vec::new(),
    };
    assert_eq!(run(&excluded).removed, 1);
    assert_eq!(run(&Exclusions::default()).added, 1);
}

#[test]
fn an_unreachable_folder_changes_nothing_and_says_so() {
    let folder = three_files("service-unreachable");
    let mut store = Store::open_in_memory().unwrap();
    index(&mut store, &folder, usize::MAX);

    // The drive is unplugged, or the folder moved: its files stay.
    let moved = folder.with_file_name("catchword-test-service-unreachable-moved");
    let _ = fs::remove_dir_all(&moved);
    fs::rename(&folder, &moved).unwrap();
    let cutter = Cutter::for_model(&Model::Unavailable("not needed".into()));
    let error = index_folder(
        &mut store,
        &folder,
        &Exclusions::default(),
        &cutter,
        &Worker::NextToProgram,
        &Limits::default(),
        |_, _| true,
    )
    .unwrap_err();
    assert!(error.downcast_ref::<Unreachable>().is_some(), "{error:#}");
    assert!(error.to_string().contains("cannot open folder"));
    assert_eq!(store.counts().unwrap().files, 3);

    fs::rename(&moved, &folder).unwrap();
    assert_eq!(index(&mut store, &folder, usize::MAX).unchanged, 3);
}

/// Run `attrib` on a file: `+o` marks it offline, as a cloud-only file is.
#[cfg(windows)]
fn attrib(flag: &str, file: &Path) {
    let status = std::process::Command::new("attrib")
        .arg(flag)
        .arg(file)
        .status()
        .unwrap();
    assert!(status.success());
}

#[cfg(windows)]
#[test]
fn a_cloud_only_file_is_never_opened_and_is_read_once_it_is_here() {
    let folder = three_files("service-cloud-only");
    let online = folder.join("online.txt");
    fs::write(&online, "kept in the cloud until opened").unwrap();
    attrib("+o", &online);
    let mut store = Store::open_in_memory().unwrap();

    let first = index(&mut store, &folder, usize::MAX);
    assert_eq!(first.added, 3);
    assert_eq!(first.not_indexed.len(), 1);
    assert_eq!(first.not_indexed[0].1, Reason::CloudOnly);
    // Recorded once: the next run does not write it again.
    let second = index(&mut store, &folder, usize::MAX);
    assert_eq!((second.known_problems, second.not_indexed.len()), (1, 1));

    // Kept on this computer now: same size and date, but read.
    attrib("-o", &online);
    let third = index(&mut store, &folder, usize::MAX);
    assert_eq!((third.added, third.not_indexed.len()), (1, 0));
    assert!(store.problems().unwrap().is_empty());
}

#[cfg(windows)]
#[test]
fn files_in_a_sub_folder_that_cannot_be_read_are_kept() {
    let folder = three_files("service-unreadable");
    let locked = folder.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(
        locked.join("inside.txt"),
        "a note in a folder that may lock",
    )
    .unwrap();
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(index(&mut store, &folder, usize::MAX).added, 4);

    let user = std::env::var("USERNAME").unwrap();
    let icacls = |args: &[&str]| {
        let status = std::process::Command::new("icacls")
            .arg(&locked)
            .args(args)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    };
    icacls(&["/deny", &format!("{user}:(RD)")]);
    let run = index(&mut store, &folder, usize::MAX);
    icacls(&["/remove:d", &user]);
    assert_eq!(run.removed, 0);
    assert_eq!(store.counts().unwrap().files, 4);
}

#[test]
fn a_pause_stops_a_long_file_part_way_and_keeps_nothing_of_it() {
    let folder = scratch_folder("service-pause-in-file");
    let long: Vec<String> = (0..10_000).map(|n| format!("word{n}")).collect();
    fs::write(folder.join("long.txt"), long.join(" ")).unwrap();
    let mut store = Store::open_in_memory().unwrap();
    let cutter = Cutter::for_model(&Model::Unavailable("not needed".into()));
    // The first question is before the file; the second comes while it is
    // being cut, and is answered no, as after a pause.
    let mut asked = 0;
    let report = index_folder(
        &mut store,
        &folder,
        &Exclusions::default(),
        &cutter,
        &Worker::NextToProgram,
        &Limits::default(),
        |_, _| {
            asked += 1;
            asked < 2
        },
    )
    .unwrap();
    assert!(report.stopped);
    assert_eq!(asked, 2);
    assert_eq!(store.counts().unwrap().files, 0);
    assert!(store.problems().unwrap().is_empty());

    // The next run reads it whole.
    assert_eq!(index(&mut store, &folder, usize::MAX).added, 1);
}

#[test]
fn the_meaning_stage_stops_after_the_passage_it_is_on() {
    // Any model that meets the contract will do: the stand-in is quick.
    let model = Model::Ready(std::sync::Mutex::new(Box::new(
        catchword_test_support::WordModel,
    )));
    let ready = model.ready().unwrap();
    let folder = three_files("service-meaning-pause");
    let mut store = Store::open_in_memory().unwrap();
    index_folder(
        &mut store,
        &folder,
        &Exclusions::default(),
        &Cutter::for_model(&model),
        &Worker::NextToProgram,
        &Limits::default(),
        |_, _| true,
    )
    .unwrap();
    // Asked before the batch, then after each passage: the third answer is
    // no, so two passages are embedded and kept.
    let mut asked = 0;
    let report = embed_missing(&mut store, ready, |_, _| {
        asked += 1;
        asked < 3
    })
    .unwrap();
    assert!(report.stopped);
    assert_eq!(store.counts().unwrap().vectors, 2);
}
