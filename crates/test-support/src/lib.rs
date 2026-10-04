//! Test helpers: PDFs written by code, scratch folders, a stand-in
//! embedding model, and the conformance suites every extractor and model
//! must pass (MNT-2).
//!
//! Test documents are generated, not committed, so every test file is
//! redistributable and contains no personal data.

pub mod conformance;
mod word_model;

pub use word_model::{WordModel, WORD_MODEL};

use std::env;
use std::fs;
use std::path::PathBuf;

/// An empty folder of the caller's own under the system temp folder.
pub fn scratch_folder(name: &str) -> PathBuf {
    let folder = env::temp_dir().join(format!("catchword-test-{name}"));
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).unwrap();
    folder
}

/// A PDF with one page per entry, each showing that text in Helvetica.
/// Lines are split on `\n`. An empty entry makes a page with no text at all,
/// like a scan. Text must be ASCII.
pub fn pdf(pages: &[&str]) -> Vec<u8> {
    write_pdf(pages, None)
}

/// A one-page PDF that asks for a password. Its encryption entry is valid in
/// form, but no password unlocks it, so a reader must refuse it.
pub fn encrypted_pdf() -> Vec<u8> {
    let o = "41".repeat(32);
    let u = "42".repeat(32);
    let encrypt = format!("<< /Filter /Standard /V 1 /R 2 /O <{o}> /U <{u}> /P -44 >>");
    write_pdf(&["secret text"], Some(&encrypt))
}

fn write_pdf(pages: &[&str], encrypt: Option<&str>) -> Vec<u8> {
    assert!(
        pages.iter().all(|page| page.is_ascii()),
        "text must be ASCII"
    );

    // Objects 1 to 3 are the catalog, the page list and the font; then a
    // page and its content for each page; then the encryption entry.
    let page_ids: Vec<usize> = (0..pages.len()).map(|i| 4 + 2 * i).collect();
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    for (page, id) in pages.iter().zip(&page_ids) {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            id + 1
        ));
        let content = content_stream(page);
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ));
    }
    if let Some(encrypt) = encrypt {
        objects.push(encrypt.to_string());
    }

    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{object}\nendobj\n", index + 1).bytes());
    }
    let xref_at = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    let mut trailer = format!("<< /Size {} /Root 1 0 R", objects.len() + 1);
    if encrypt.is_some() {
        let id = "0123456789abcdef0123456789abcdef";
        trailer.push_str(&format!(
            " /Encrypt {} 0 R /ID [<{id}> <{id}>]",
            objects.len()
        ));
    }
    out.extend(format!("trailer\n{trailer} >>\nstartxref\n{xref_at}\n%%EOF\n").bytes());
    out
}

/// Drawing commands that show each line of `text` at the top of the page.
fn content_stream(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let escaped = line
                .replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)");
            format!("({escaped}) Tj T*")
        })
        .collect();
    format!("BT /F1 12 Tf 14 TL 72 720 Td {} ET", lines.join(" "))
}
