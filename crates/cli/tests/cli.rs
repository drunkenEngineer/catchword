//! The command-line tool end to end, with the real worker and PDFium.
//!
//! Run with `cargo test --workspace`, which also builds the worker program
//! this tool looks for next to itself.

use std::fs;
use std::path::Path;
use std::process::Command;

use catchword_test_support::{encrypted_pdf, pdf, scratch_folder};

/// Run the tool and return what it printed. Fails the test if it failed.
fn catchword(db: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_catchword"))
        .arg("--db")
        .arg(db)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "catchword {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn indexes_text_and_pdf_and_finds_pdf_passages_by_page() {
    let folder = scratch_folder("cli-mixed");
    let docs = folder.join("docs");
    fs::create_dir(&docs).unwrap();
    fs::write(
        docs.join("notes.txt"),
        "The termination notice period is three months.",
    )
    .unwrap();
    fs::write(
        docs.join("letter.pdf"),
        pdf(&["Dear customer,", "Your tax refund was approved."]),
    )
    .unwrap();
    fs::write(docs.join("scan.pdf"), pdf(&[""])).unwrap();
    fs::write(docs.join("locked.pdf"), encrypted_pdf()).unwrap();
    let db = folder.join("index.db");
    let docs = docs.to_str().unwrap();

    let report = catchword(&db, &["index", docs]);
    assert!(report.contains("new or changed: 2"), "{report}");
    assert!(report.contains("skipped: 2"), "{report}");
    assert!(report.contains("failed: 0"), "{report}");
    assert!(report.contains("scan.pdf: no text layer"), "{report}");
    assert!(
        report.contains("locked.pdf: protected by a password"),
        "{report}"
    );

    let found = catchword(&db, &["search", "tax", "refund"]);
    assert!(found.contains("letter.pdf  page 2"), "{found}");
    let found = catchword(&db, &["search", "notice", "period"]);
    assert!(found.contains("notes.txt  lines 1-1"), "{found}");

    // Nothing changed: nothing is read again.
    let report = catchword(&db, &["index", docs]);
    assert!(report.contains("new or changed: 0"), "{report}");
    assert!(report.contains("unchanged: 2"), "{report}");
}

#[test]
fn meaning_search_finds_passages_that_share_no_words_with_the_query() {
    let folder = scratch_folder("cli-meaning");
    let docs = folder.join("docs");
    fs::create_dir(&docs).unwrap();
    fs::write(
        docs.join("reimbursement.txt"),
        "Your income tax reimbursement was approved and will be paid next week.",
    )
    .unwrap();
    fs::write(
        docs.join("remboursement.txt"),
        "Votre remboursement d'impôt a été approuvé.",
    )
    .unwrap();
    fs::write(
        docs.join("cat.txt"),
        "The cat slept on the warm windowsill all afternoon.",
    )
    .unwrap();
    let db = folder.join("index.db");
    let docs = docs.to_str().unwrap();

    let report = catchword(&db, &["index", docs]);
    assert!(
        report.contains("searchable by meaning: 3 of 3 passages"),
        "{report}"
    );

    // "refund" is in none of the files: only meaning can find them, and the
    // two about refunds, in English and in French, come before the cat.
    let found = catchword(&db, &["search", "refund"]);
    let lines: Vec<&str> = found
        .lines()
        .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
        .collect();
    assert!(lines.iter().all(|l| l.ends_with("(meaning)")), "{found}");
    let about_refunds =
        |l: &&str| l.contains("reimbursement.txt") || l.contains("remboursement.txt");
    assert!(lines[..2].iter().all(about_refunds), "{found}");
    assert!(lines[2].contains("cat.txt"), "{found}");

    // Found both ways ranks first.
    let found = catchword(&db, &["search", "tax"]);
    assert!(
        found.contains("reimbursement.txt  lines 1-1  (words and meaning)"),
        "{found}"
    );

    // A second run has nothing left to embed.
    let report = catchword(&db, &["index", docs]);
    assert!(report.contains("unchanged: 3"), "{report}");
    assert!(
        report.contains("searchable by meaning: 3 of 3 passages"),
        "{report}"
    );
}
