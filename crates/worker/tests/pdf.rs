//! The real worker with the real PDFium library, run through the engine's
//! supervisor. Needs `sh scripts/fetch-pdfium.sh` to have been run once.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use catchword_engine::extract::{run, Limits, Outcome, Reason};
use catchword_test_support::{encrypted_pdf, pdf, scratch_folder};

fn worker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_catchword-worker"))
}

fn limits() -> Limits {
    Limits {
        timeout: Duration::from_secs(30),
        ..Limits::default()
    }
}

fn write(folder: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = folder.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn extract(file: &Path, limits: &Limits) -> Outcome {
    run(worker(), file, limits).unwrap()
}

#[test]
fn reads_the_text_of_each_page() {
    let folder = scratch_folder("worker-pages");
    let file = write(
        &folder,
        "letter.pdf",
        &pdf(&[
            "Your tax refund was approved.\nReference INV-2026-0042",
            "The notice period is (three) months.",
        ]),
    );
    let Outcome::Pages(pages) = extract(&file, &limits()) else {
        panic!("no pages; is PDFium in vendor/pdfium? Run sh scripts/fetch-pdfium.sh");
    };
    assert_eq!(pages.len(), 2);
    assert!(pages[0].contains("tax refund was approved"), "{pages:?}");
    assert!(pages[0].contains("INV-2026-0042"), "{pages:?}");
    assert!(
        pages[1].contains("notice period is (three) months"),
        "{pages:?}"
    );
}

#[test]
fn a_pdf_without_text_needs_ocr() {
    let folder = scratch_folder("worker-blank");
    let file = write(&folder, "scan.pdf", &pdf(&["", ""]));
    assert_eq!(
        extract(&file, &limits()),
        Outcome::NotIndexed(Reason::NeedsOcr)
    );
}

#[test]
fn a_password_protected_pdf_is_skipped() {
    let folder = scratch_folder("worker-encrypted");
    let file = write(&folder, "locked.pdf", &encrypted_pdf());
    assert_eq!(
        extract(&file, &limits()),
        Outcome::NotIndexed(Reason::Encrypted)
    );
}

#[test]
fn a_file_that_is_not_a_pdf_is_damaged() {
    let folder = scratch_folder("worker-garbage");
    let noise: Vec<u8> = (0..4096u32).map(|i| (i * 7919 % 251) as u8).collect();
    let file = write(&folder, "noise.pdf", &noise);
    assert_eq!(
        extract(&file, &limits()),
        Outcome::NotIndexed(Reason::Damaged)
    );
}

#[test]
fn a_cut_off_pdf_is_damaged() {
    let folder = scratch_folder("worker-cut");
    let whole = pdf(&["some text"]);
    let file = write(&folder, "cut.pdf", &whole[..30]);
    assert_eq!(
        extract(&file, &limits()),
        Outcome::NotIndexed(Reason::Damaged)
    );
}

#[test]
fn too_many_pages_is_too_large() {
    let folder = scratch_folder("worker-many-pages");
    let file = write(&folder, "long.pdf", &pdf(&["one", "two", "three"]));
    let limits = Limits {
        max_pages: 2,
        ..limits()
    };
    assert_eq!(
        extract(&file, &limits),
        Outcome::NotIndexed(Reason::TooLarge)
    );
}

#[test]
fn too_much_text_is_too_large() {
    let folder = scratch_folder("worker-much-text");
    let file = write(
        &folder,
        "wordy.pdf",
        &pdf(&["far more than twenty bytes of text"]),
    );
    let limits = Limits {
        max_text_bytes: 20,
        ..limits()
    };
    assert_eq!(
        extract(&file, &limits),
        Outcome::NotIndexed(Reason::TooLarge)
    );
}

#[test]
fn unusual_file_names_reach_the_worker_intact() {
    let folder = scratch_folder("worker names é");
    let file = write(
        &folder,
        "Résumé (final) – 2026.pdf",
        &pdf(&["curriculum vitae"]),
    );
    let Outcome::Pages(pages) = extract(&file, &limits()) else {
        panic!("not read");
    };
    assert!(pages[0].contains("curriculum vitae"));
}
