//! Indexing and search through the service, as both front ends use them.
//! The meaning-stage tests need `sh scripts/fetch-embedding.sh`.

use std::fs;
use std::path::Path;

use catchword_embed::{Copier, Embed, EmbedError};
use catchword_engine::extract::{Limits, Reason};
use catchword_engine::fuse::Found;
use catchword_engine::Exclusions;
use catchword_service::{
    embed_missing, group_by_file, index_folder, is_parked, search, search_within, search_words,
    Copies, Cutter, Model, Note, Report, Unreachable, Worker, COPIES_FROM,
};
use catchword_store::Store;
use catchword_test_support::{scratch_folder, WordModel};

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

    let stopped = embed_missing(&mut store, ready, Copies::NONE, |_, _| false).unwrap();
    assert!(stopped.stopped);
    assert_eq!(store.counts().unwrap().vectors, 0);

    let finished = embed_missing(&mut store, ready, Copies::NONE, |_, _| true).unwrap();
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
fn a_search_by_words_alone_needs_no_model_and_finds_no_meaning() {
    let model = Model::Ready(std::sync::Mutex::new(Box::new(
        catchword_test_support::WordModel,
    )));
    let folder = three_files("service-words-first");
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
    embed_missing(&mut store, model.ready().unwrap(), Copies::NONE, |_, _| {
        true
    })
    .unwrap();
    let filter = catchword_store::Filter::default();

    // The full search finds the lease by meaning as well as by words.
    let full = search_within(&store, &model, "lease notice", &filter).unwrap();
    assert!(full
        .results
        .iter()
        .any(|(_, found)| matches!(found, Found::Meaning | Found::Both)));

    let words = search_words(&store, "lease notice", &filter).unwrap();
    assert!(words.notes.is_empty());
    assert!(!words.results.is_empty());
    assert!(words
        .results
        .iter()
        .all(|(_, found)| matches!(found, Found::Keyword | Found::Name)));
    assert!(words.results[0].0.path.ends_with("a.txt"));
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
    let report = embed_missing(&mut store, ready, Copies::NONE, |_, _| {
        asked += 1;
        asked < 3
    })
    .unwrap();
    assert!(report.stopped);
    assert_eq!(store.counts().unwrap().vectors, 2);
}

// Copies of the model for a long meaning stage (ADR-25), with the quick
// stand-in model, whose copies behave as each test needs.

/// `count` files of different short notes, one passage each, indexed into a
/// new store with the stand-in model's cutter.
fn many_notes(name: &str, count: usize) -> Store {
    let folder = scratch_folder(name);
    let words = [
        "lease", "notice", "invoice", "refund", "tax", "court", "letter", "salary", "tenant",
        "audit", "budget", "claim", "policy", "editor", "source", "deadline",
    ];
    for n in 0..count {
        let text: Vec<&str> = (0..12)
            .map(|i| words[(n * 7 + i * 5 + n / 16) % 16])
            .collect();
        fs::write(
            folder.join(format!("note-{n}.txt")),
            format!("note {n} {}", text.join(" ")),
        )
        .unwrap();
    }
    let mut store = Store::open_in_memory().unwrap();
    index_folder(
        &mut store,
        &folder,
        &Exclusions::default(),
        &Cutter::for_model(&model(CopyAs::Same)),
        &Worker::NextToProgram,
        &Limits::default(),
        |_, _| true,
    )
    .unwrap();
    store
}

/// What copying the stand-in model does.
#[derive(Clone, Copy, PartialEq)]
enum CopyAs {
    /// Makes a stand-in model.
    Same,
    /// Fails, as a damaged model file would.
    Refused,
    /// Must not happen: the test fails if a copy is made.
    Never,
    /// Makes a stand-in model that fails from its 50th passage on.
    WearsOut,
}

/// The stand-in model, copied as `copy_as` says.
struct Stand {
    copy_as: CopyAs,
    /// Passages embedded, and whether it fails from the 50th on.
    used: usize,
    wears_out: bool,
}

impl Embed for Stand {
    fn manifest(&self) -> &catchword_embed::ModelManifest {
        &catchword_test_support::WORD_MODEL
    }

    fn tokenizer(&self) -> Box<dyn catchword_engine::Tokenizer + Send + Sync> {
        Box::new(catchword_engine::WordTokenizer)
    }

    fn embed_passages(&mut self, passages: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.used += 1;
        if self.wears_out && self.used >= 50 {
            return Err(EmbedError::Runtime("worn out".into()));
        }
        WordModel.embed_passages(passages)
    }

    fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, EmbedError> {
        WordModel.embed_query(query)
    }

    fn copier(&self) -> Option<Box<dyn Copier>> {
        Some(Box::new(StandCopier(self.copy_as)))
    }
}

struct StandCopier(CopyAs);

impl Copier for StandCopier {
    fn copy(&self, _: catchword_service::Threads) -> Result<Box<dyn Embed>, EmbedError> {
        match self.0 {
            CopyAs::Same | CopyAs::WearsOut => Ok(Box::new(Stand {
                copy_as: self.0,
                used: 0,
                wears_out: self.0 == CopyAs::WearsOut,
            })),
            CopyAs::Refused => Err(EmbedError::Runtime("no copies here".into())),
            CopyAs::Never => panic!("a copy was made for a short stage"),
        }
    }
}

fn model(copy_as: CopyAs) -> Model {
    Model::Ready(std::sync::Mutex::new(Box::new(Stand {
        copy_as,
        used: 0,
        wears_out: false,
    })))
}

fn copies(count: usize) -> Copies {
    Copies {
        count,
        threads: catchword_service::Threads::RUNTIME_DEFAULT,
    }
}

/// The nearest passages to a few queries, by file name, text and score:
/// the same vectors give the same.
fn nearest(store: &Store) -> Vec<(String, String, i64)> {
    ["lease notice", "tax refund court", "budget deadline editor"]
        .iter()
        .flat_map(|query| {
            let vector = WordModel.embed_query(query).unwrap();
            store.search_vector(&vector, 10).unwrap()
        })
        .map(|hit| {
            let name = Path::new(&hit.path).file_name().unwrap();
            let text = store.passage_text(hit.passage_id).unwrap().unwrap();
            // Scores to six places: equal vectors, equal scores.
            let score = (hit.score * 1e6).round() as i64;
            (name.to_string_lossy().into_owned(), text, score)
        })
        .collect()
}

#[test]
fn a_long_meaning_stage_uses_copies_and_gets_the_same_vectors() {
    let notes = COPIES_FROM as usize + 44;
    let shared = model(CopyAs::Same);

    let mut alone = many_notes("service-copies-alone", notes);
    let report = embed_missing(&mut alone, shared.ready().unwrap(), Copies::NONE, |_, _| {
        true
    })
    .unwrap();
    assert_eq!(report.copies, 0);

    let mut side_by_side = many_notes("service-copies-three", notes);
    let report = embed_missing(
        &mut side_by_side,
        shared.ready().unwrap(),
        copies(3),
        |_, _| true,
    )
    .unwrap();
    assert_eq!(report.copies, 3);
    let counts = side_by_side.counts().unwrap();
    assert_eq!(counts.passages, notes as i64);
    assert_eq!(counts.vectors, counts.passages);
    let expected = nearest(&alone);
    assert_eq!(expected.len(), 30);
    assert_eq!(nearest(&side_by_side), expected);
    // The copies embedded it all: the shared model stayed free for searches.
    let mut shared = shared.ready().unwrap().lock().unwrap();
    assert_eq!(shared.embed_passages(&["one more"]).unwrap().len(), 1);
}

#[test]
fn a_short_meaning_stage_makes_no_copies() {
    let mut store = many_notes("service-copies-short", 20);
    let shared = model(CopyAs::Never);
    let report =
        embed_missing(&mut store, shared.ready().unwrap(), copies(3), |_, _| true).unwrap();
    assert_eq!(report.copies, 0);
    let counts = store.counts().unwrap();
    assert_eq!(counts.vectors, counts.passages);
}

#[test]
fn copies_that_cannot_be_made_leave_the_work_to_the_shared_model() {
    let mut store = many_notes("service-copies-refused", COPIES_FROM as usize);
    let shared = model(CopyAs::Refused);
    let report =
        embed_missing(&mut store, shared.ready().unwrap(), copies(3), |_, _| true).unwrap();
    assert_eq!(report.copies, 0);
    let counts = store.counts().unwrap();
    assert_eq!(counts.vectors, counts.passages);
}

#[test]
fn a_pause_while_copies_are_made_stops_before_the_next_one() {
    let mut store = many_notes("service-copies-pause-early", COPIES_FROM as usize);
    let shared = model(CopyAs::Same);
    // Asked before each of the four copies: the second answer is no.
    let mut asked = 0;
    let report = embed_missing(&mut store, shared.ready().unwrap(), copies(4), |_, _| {
        asked += 1;
        asked < 2
    })
    .unwrap();
    assert!(report.stopped);
    assert_eq!(asked, 2);
    assert_eq!(store.counts().unwrap().vectors, 0);
}

#[test]
fn a_pause_with_copies_keeps_what_they_embedded_and_the_next_run_carries_on() {
    let mut store = many_notes("service-copies-pause", COPIES_FROM as usize + 100);
    let shared = model(CopyAs::Same);
    // Asked before each of the four copies, before the first batch, then
    // after each passage: the 45th answer is no, after 40 passages. What
    // the copies finished meanwhile is kept too (the stand-in is quick, so
    // that can be many), but nothing past the batch of 4 x 32 passages.
    let mut asked = 0;
    let report = embed_missing(&mut store, shared.ready().unwrap(), copies(4), |_, _| {
        asked += 1;
        asked < 45
    })
    .unwrap();
    assert!(report.stopped);
    assert_eq!(report.copies, 4);
    let kept = store.counts().unwrap().vectors;
    assert!((40..=128).contains(&kept), "{kept}");

    let report =
        embed_missing(&mut store, shared.ready().unwrap(), copies(4), |_, _| true).unwrap();
    assert!(!report.stopped);
    let counts = store.counts().unwrap();
    assert_eq!(counts.vectors, counts.passages);
}

#[test]
fn a_copy_that_fails_ends_the_stage_with_its_error() {
    let mut store = many_notes("service-copies-failing", COPIES_FROM as usize + 300);
    let shared = model(CopyAs::WearsOut);
    let error =
        embed_missing(&mut store, shared.ready().unwrap(), copies(2), |_, _| true).unwrap_err();
    assert!(format!("{error:#}").contains("worn out"), "{error:#}");
    // As with the shared model alone: the batch that failed is not saved,
    // and the next run embeds it.
    let counts = store.counts().unwrap();
    assert!(counts.vectors < counts.passages);
}
