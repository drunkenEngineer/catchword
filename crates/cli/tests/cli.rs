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
