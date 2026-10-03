//! Indexing and search through the service, as both front ends use them.
//! The meaning-stage tests need `sh scripts/fetch-embedding.sh`.

use std::fs;
use std::path::Path;

use catchword_engine::extract::Limits;
use catchword_service::{
    embed_missing, group_by_file, index_folder, search, Cutter, Model, Note, Worker,
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

fn index(store: &mut Store, folder: &Path, stop_after: usize) -> catchword_service::Report {
    let cutter = Cutter::for_model(&Model::Unavailable("not needed".into()));
    index_folder(
        store,
        folder,
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
    let model = Model::load();
    let Some(ready) = model.ready() else {
        panic!("no model; run sh scripts/fetch-embedding.sh");
    };
    let folder = three_files("service-meaning");
    let mut store = Store::open_in_memory().unwrap();
    let cutter = Cutter::for_model(&model);
    index_folder(
        &mut store,
        &folder,
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
