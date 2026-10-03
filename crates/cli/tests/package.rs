//! The packaged files work together: beside the program, the PDF reader,
//! PDFium, ONNX Runtime and the model are found and used, with nothing
//! taken from the source tree. Run after `sh scripts/package-msix.sh`:
//!
//! `cargo test -p catchword --test package -- --ignored`

use std::fs;
use std::path::Path;
use std::process::Command;

use catchword_test_support::{pdf, scratch_folder};

/// Copy a folder and everything in it.
fn copy_folder(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
#[ignore = "needs target/package/Catchword: run sh scripts/package-msix.sh"]
fn the_packaged_files_read_pdfs_and_search_by_meaning() {
    let layout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/package/Catchword");
    assert!(
        layout.join("AppxManifest.xml").is_file(),
        "run sh scripts/package-msix.sh first"
    );
    // The command-line tool stands in for the app: it finds its files the
    // same way, and can be run without a window.
    let folder = scratch_folder("package-layout");
    let installed = folder.join("installed");
    copy_folder(&layout, &installed);
    let tool = installed.join(format!("catchword{}", std::env::consts::EXE_SUFFIX));
    fs::copy(env!("CARGO_BIN_EXE_catchword"), &tool).unwrap();

    let docs = folder.join("docs");
    fs::create_dir(&docs).unwrap();
    fs::write(
        docs.join("letter.pdf"),
        pdf(&["Dear customer,", "Your tax refund was approved."]),
    )
    .unwrap();
    let run = |args: &[&str]| -> String {
        let output = Command::new(&tool)
            .arg("--db")
            .arg(folder.join("index.db"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let report = run(&["index", docs.to_str().unwrap()]);
    assert!(report.contains("new or changed: 1"), "{report}");
    assert!(report.contains("failed: 0"), "{report}");
    assert!(!report.contains("meaning search is off"), "{report}");

    // Shares no word with the letter: only meaning finds it.
    let found = run(&["search", "money", "back", "from", "the", "government"]);
    assert!(found.contains("letter.pdf  page 2"), "{found}");
}
