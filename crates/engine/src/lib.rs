//! Catchword engine: scanning, reading and splitting documents.
//!
//! Privacy rule: this crate must never contain network code.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub mod extract;

use extract::Reason;

/// A piece of a document, small enough to index and later to embed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passage {
    /// Position of the passage within its document, starting at 0.
    pub ordinal: u32,
    /// The page the passage is on, starting at 1. None for plain text.
    pub page: Option<u32>,
    /// First line the passage touches, starting at 1. On a page, counted
    /// from the top of that page.
    pub start_line: u32,
    /// Last line the passage touches.
    pub end_line: u32,
    pub text: String,
}

/// The kinds of document this version can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    /// Plain text and Markdown, read by the engine itself (ADR-17).
    Text,
    /// PDF, read only by the extraction worker.
    Pdf,
}

/// What the scanner learns about a file without opening it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    pub path: PathBuf,
    pub size: u64,
    pub modified_secs: i64,
}

/// The kind of document a file is, judged by its extension, or None when
/// this version cannot read it.
pub fn document_kind(path: &Path) -> Option<DocumentKind> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("txt" | "md" | "markdown") => Some(DocumentKind::Text),
        Some("pdf") => Some(DocumentKind::Pdf),
        _ => None,
    }
}

/// Turn a folder the user chose into the absolute, real path that a scan starts from.
///
/// Links and `..` are resolved. On Windows the result is the plain form
/// (`C:\Users\...`), not the `\\?\C:\Users\...` form the standard library
/// returns. The `\\?\` form is kept only where a plain path would not mean the
/// same folder, such as a path longer than 260 characters.
pub fn resolve_folder(folder: &Path) -> io::Result<PathBuf> {
    dunce::canonicalize(folder)
}

/// List the files under `root`, sorted by path.
///
/// Links are not followed, so a scan can never leave the folder the user chose.
/// Hidden files and folders (names starting with a dot) are skipped.
pub fn scan(root: &Path) -> Vec<FileMeta> {
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'));
    let mut found = Vec::new();
    for entry in walker.filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let modified_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        found.push(FileMeta {
            path: entry.into_path(),
            size: meta.len(),
            modified_secs,
        });
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

/// Read a plain-text or Markdown file and return its text with the hash of
/// its bytes. Files over `max_bytes` are refused.
///
/// Invalid UTF-8 is replaced, not rejected, and control characters are
/// dropped. Proper encoding detection comes later.
pub fn read_text(path: &Path, max_bytes: u64) -> Result<(String, String), Reason> {
    let file = fs::File::open(path).map_err(|_| Reason::CannotOpen)?;
    // One byte past the limit is enough to know it is too large, even if the
    // file grew after the scan.
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| Reason::CannotOpen)?;
    if bytes.len() as u64 > max_bytes {
        return Err(Reason::TooLarge);
    }
    let hash = content_hash(&bytes);
    let decoded = String::from_utf8_lossy(&bytes);
    let text = without_controls(decoded.strip_prefix('\u{feff}').unwrap_or(&decoded));
    Ok((text, hash))
}

/// Remove control characters other than tab and line breaks. Document text
/// is untrusted: some control characters, such as terminal escape codes,
/// could act on the screen that shows them.
pub fn without_controls(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

/// SHA-256 of the file bytes, as lowercase hex. Identical files share one hash.
pub fn content_hash(bytes: &[u8]) -> String {
    to_hex(&Sha256::digest(bytes))
}

/// Like `content_hash`, but reads the file in pieces, so a large file is
/// never held in memory whole.
pub fn hash_file(path: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    io::copy(&mut fs::File::open(path)?, &mut hasher)?;
    Ok(to_hex(&hasher.finalize()))
}

fn to_hex(digest: &[u8]) -> String {
    use std::fmt::Write;

    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing to a String cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Split text into passages of at most `max_words` words.
///
/// Neighbouring passages share `overlap_words` words, so a sentence cut at a
/// boundary is still whole in one of them. Words stand in for model tokens
/// until the embedding step sets the real limit.
pub fn chunk(text: &str, max_words: usize, overlap_words: usize) -> Vec<Passage> {
    assert!(max_words > 0, "max_words must be positive");
    assert!(
        overlap_words < max_words,
        "overlap must be smaller than the passage"
    );

    let mut words: Vec<(u32, &str)> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for word in line.split_whitespace() {
            words.push((index as u32 + 1, word));
        }
    }

    let mut passages = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let end = (start + max_words).min(words.len());
        let slice = &words[start..end];
        passages.push(Passage {
            ordinal: passages.len() as u32,
            page: None,
            start_line: slice[0].0,
            end_line: slice[slice.len() - 1].0,
            text: slice.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" "),
        });
        if end == words.len() {
            break;
        }
        start = end - overlap_words;
    }
    passages
}

/// Split each page into passages on its own, so every passage lies on one
/// page and carries its number. Ordinals run on across pages.
pub fn chunk_pages(pages: &[String], max_words: usize, overlap_words: usize) -> Vec<Passage> {
    let mut passages = Vec::new();
    for (index, text) in pages.iter().enumerate() {
        for passage in chunk(text, max_words, overlap_words) {
            passages.push(Passage {
                ordinal: passages.len() as u32,
                page: Some(index as u32 + 1),
                ..passage
            });
        }
    }
    passages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_keeps_every_word_and_respects_the_limit() {
        let text = (1..=95)
            .map(|n| format!("w{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        let passages = chunk(&text, 20, 5);
        assert!(passages
            .iter()
            .all(|p| p.text.split_whitespace().count() <= 20));
        assert!(passages[0].text.starts_with("w1 "));
        assert!(passages[1].text.starts_with("w16 "));
        assert!(passages.last().unwrap().text.ends_with("w95"));
        let ordinals: Vec<u32> = passages.iter().map(|p| p.ordinal).collect();
        assert_eq!(ordinals, (0..passages.len() as u32).collect::<Vec<_>>());
    }

    #[test]
    fn chunk_tracks_line_numbers() {
        let passages = chunk("one two\n\nthree four\nfive", 3, 1);
        assert_eq!((passages[0].start_line, passages[0].end_line), (1, 3));
        assert_eq!(passages[1].start_line, 3);
        assert_eq!(passages.last().unwrap().end_line, 4);
    }

    #[test]
    fn empty_text_gives_no_passages() {
        assert!(chunk("  \n\n ", 10, 2).is_empty());
    }

    #[test]
    fn identical_bytes_share_a_hash() {
        assert_eq!(content_hash(b"same"), content_hash(b"same"));
        assert_ne!(content_hash(b"same"), content_hash(b"different"));
        assert_eq!(content_hash(b"").len(), 64);
    }

    /// An empty folder of the test's own under the system temp folder.
    fn test_folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("catchword-engine-test-{name}"));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn resolved_folder_is_absolute_and_plain() {
        let folder = test_folder("plain");
        fs::create_dir(folder.join("sub")).unwrap();

        let resolved = resolve_folder(&folder.join("sub").join("..")).unwrap();
        assert!(resolved.is_absolute());
        assert!(resolved.ends_with("catchword-engine-test-plain"));
        assert!(
            !resolved.to_string_lossy().starts_with(r"\\?\"),
            "{}",
            resolved.display()
        );
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn long_paths_keep_working() {
        let folder = test_folder("long");
        // Six 50-character names put the file past Windows' old 260-character limit.
        let mut deep = folder.clone();
        for _ in 0..6 {
            deep.push("a-folder-name-of-exactly-fifty-characters-in-total");
        }
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("note.txt"), "deep inside").unwrap();

        // A short folder with a long path below it.
        let found = scan(&resolve_folder(&folder).unwrap());
        assert_eq!(found.len(), 1);
        assert!(found[0].path.as_os_str().len() > 260);
        assert_eq!(read_text(&found[0].path, 100).unwrap().0, "deep inside");

        // A folder whose own path is long.
        let found = scan(&resolve_folder(&deep).unwrap());
        assert_eq!(found.len(), 1);
        assert_eq!(read_text(&found[0].path, 100).unwrap().0, "deep inside");
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn text_markdown_and_pdf_are_supported() {
        let kind = |name: &str| document_kind(Path::new(name));
        assert_eq!(kind("notes/Plan.MD"), Some(DocumentKind::Text));
        assert_eq!(kind("a.txt"), Some(DocumentKind::Text));
        assert_eq!(kind("scan_0042.PDF"), Some(DocumentKind::Pdf));
        assert_eq!(kind("report.docx"), None);
        assert_eq!(kind("no_extension"), None);
    }

    #[test]
    fn read_text_refuses_files_over_the_limit() {
        let folder = test_folder("text-limit");
        let file = folder.join("big.txt");
        fs::write(&file, "0123456789").unwrap();
        assert!(read_text(&file, 10).is_ok());
        assert_eq!(read_text(&file, 9), Err(Reason::TooLarge));
        assert_eq!(
            read_text(&folder.join("missing.txt"), 10),
            Err(Reason::CannotOpen)
        );
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn read_text_drops_control_characters_and_the_byte_order_mark() {
        let folder = test_folder("text-controls");
        let file = folder.join("note.txt");
        let bytes = "\u{feff}line one\r\n\u{1b}[31mred\u{7}\tend".as_bytes();
        fs::write(&file, bytes).unwrap();
        let (text, hash) = read_text(&file, 100).unwrap();
        assert_eq!(text, "line one\r\n[31mred\tend");
        // The hash is of the bytes on disk, not of the cleaned text.
        assert_eq!(hash, content_hash(bytes));
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn hashing_a_file_matches_hashing_its_bytes() {
        let folder = test_folder("hash-file");
        let file = folder.join("data.bin");
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(&file, &bytes).unwrap();
        assert_eq!(hash_file(&file).unwrap(), content_hash(&bytes));
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn chunk_pages_numbers_pages_and_keeps_ordinals_running() {
        let pages = vec![
            "one two three".to_string(),
            String::new(),
            "four five".to_string(),
        ];
        let passages = chunk_pages(&pages, 2, 1);
        let found: Vec<(u32, Option<u32>, &str)> = passages
            .iter()
            .map(|p| (p.ordinal, p.page, p.text.as_str()))
            .collect();
        assert_eq!(
            found,
            vec![
                (0, Some(1), "one two"),
                (1, Some(1), "two three"),
                (2, Some(3), "four five"),
            ]
        );
        assert!(chunk("plain text", 5, 1).iter().all(|p| p.page.is_none()));
    }
}
