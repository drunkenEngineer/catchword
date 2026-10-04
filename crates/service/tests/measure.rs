//! Measurements against section 14 of the specification, not checks:
//! run them in a release build, on the machine to be measured.
//!
//! `cargo test --release -p catchword-service --test measure -- --ignored --nocapture`

use std::fs;
use std::time::Instant;

use catchword_engine::extract::Limits;
use catchword_engine::Exclusions;
use catchword_service::{index_folder, Cutter, Model, Worker};
use catchword_store::Store;
use catchword_test_support::scratch_folder;

#[test]
#[ignore = "a measurement: 10,000 files"]
fn measure_a_library_of_ten_thousand_files() {
    let folder = scratch_folder("measure-ten-thousand");
    let words = [
        "lease", "notice", "period", "invoice", "payment", "contract", "tenant", "landlord",
        "months", "refund", "tax", "letter", "bank", "account", "the", "of", "and", "within",
        "days", "after", "meeting", "report", "budget", "client", "project", "deadline",
    ];
    // 100 folders of 100 notes, each about 150 words: a small office's files.
    for f in 0..100 {
        let sub = folder.join(format!("folder-{f:03}"));
        fs::create_dir(&sub).unwrap();
        for n in 0..100 {
            // Each note its own text: a simple random walk through the words.
            let mut state = (f * 100 + n) as u64 * 2_654_435_761 + 1;
            let mut text = format!("note {f} {n}");
            for _ in 0..150 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                text.push(' ');
                text.push_str(words[(state >> 33) as usize % words.len()]);
            }
            fs::write(sub.join(format!("note-{n:03}.txt")), text).unwrap();
        }
    }
    let index = folder.with_file_name("catchword-test-measure-ten-thousand.db");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", index.display()));
    }
    let mut store = Store::open(&index).unwrap();
    // Words only: the meaning stage is measured on its own.
    let cutter = Cutter::for_model(&Model::Unavailable("measured apart".into()));
    let run = |store: &mut Store| {
        let started = Instant::now();
        let report = index_folder(
            store,
            &folder,
            &Exclusions::with_default_patterns(),
            &cutter,
            &Worker::NextToProgram,
            &Limits::default(),
            |_, _| true,
        )
        .unwrap();
        (started.elapsed(), report)
    };

    let (first, report) = run(&mut store);
    assert_eq!(report.added, 10_000);
    let (again, report) = run(&mut store);
    assert_eq!(report.unchanged, 10_000);

    let counts = store.counts().unwrap();
    drop(store);
    let bytes: u64 = ["", "-wal"]
        .iter()
        .filter_map(|suffix| fs::metadata(format!("{}{suffix}", index.display())).ok())
        .map(|meta| meta.len())
        .sum();
    println!("10,000 new files searchable by words: {first:.1?}");
    println!("scan of the same 10,000 files, unchanged: {again:.1?} (target: under 30 s)");
    println!(
        "{} passages, {:.1} KB a passage without vectors",
        counts.passages,
        bytes as f64 / 1024.0 / counts.passages as f64
    );
    assert!(again.as_secs() < 30);
}
