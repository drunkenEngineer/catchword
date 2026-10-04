//! Turning the service's results into the contract's shapes. Pure
//! functions, so they are tested without a window.

use std::path::Path;

use catchword_engine::fuse::Found;
use catchword_engine::without_controls;
use catchword_service::{is_parked, FileResults};
use catchword_store::{Hit, Problem};

use crate::contract::{FileHit, FoundBy, NotIndexed, PassageHit, Span};

/// A path as a file name and the folder it is in, ready to show. Control
/// characters are removed: names are untrusted too.
pub fn name_and_folder(path: &str) -> (String, String) {
    let path = Path::new(path);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let folder = path
        .parent()
        .map(|folder| folder.to_string_lossy().to_string())
        .unwrap_or_default();
    (without_controls(&name), without_controls(&folder))
}

/// Where in its file a passage is: a page for PDFs, lines otherwise.
pub fn location(page: Option<i64>, start_line: i64, end_line: i64) -> String {
    match page {
        Some(page) => format!("page {page}"),
        None if start_line == end_line => format!("line {start_line}"),
        None => format!("lines {start_line}–{end_line}"),
    }
}

/// Split a snippet whose matches the store marked with char(2) and char(3).
pub fn spans(snippet: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for (index, piece) in snippet.split('\u{2}').enumerate() {
        let (marked, rest) = match piece.split_once('\u{3}') {
            Some((marked, rest)) if index > 0 => (marked, rest),
            _ => ("", piece),
        };
        for (text, is_marked) in [(marked, true), (rest, false)] {
            if !text.is_empty() {
                spans.push(Span {
                    text: text.to_string(),
                    marked: is_marked,
                });
            }
        }
    }
    spans
}

fn found_by(found: Found) -> FoundBy {
    match found {
        Found::Keyword => FoundBy::Words,
        Found::Meaning => FoundBy::Meaning,
        Found::Both => FoundBy::Both,
        Found::Name => FoundBy::Name,
    }
}

pub fn passage_hit(hit: &Hit, found: Found) -> PassageHit {
    PassageHit {
        id: hit.passage_id,
        location: location(hit.page, hit.start_line, hit.end_line),
        snippet: spans(&hit.snippet),
        found: found_by(found),
    }
}

pub fn file_hits(files: Vec<FileResults>) -> Vec<FileHit> {
    files
        .into_iter()
        .map(|file| {
            let (name, folder) = name_and_folder(&file.path);
            FileHit {
                name,
                folder,
                copies: file.copies,
                modified_secs: file.modified_secs,
                passages: file
                    .passages
                    .iter()
                    .map(|(hit, found)| passage_hit(hit, *found))
                    .collect(),
            }
        })
        .collect()
}

pub fn not_indexed(problem: &Problem) -> NotIndexed {
    let (name, folder) = name_and_folder(&problem.path);
    NotIndexed {
        name,
        folder,
        reason: problem.reason.describe().to_string(),
        failed: problem.reason.is_failure(),
        parked: is_parked(problem),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use catchword_engine::extract::Reason;

    #[test]
    fn paths_are_split_and_cleaned_for_display() {
        let (name, folder) = name_and_folder("/home/ana/docs/lease\u{1b}[2J.txt");
        assert_eq!(name, "lease[2J.txt");
        assert_eq!(folder, "/home/ana/docs");
    }

    #[test]
    fn locations_read_naturally() {
        assert_eq!(location(Some(3), 1, 9), "page 3");
        assert_eq!(location(None, 4, 4), "line 4");
        assert_eq!(location(None, 4, 7), "lines 4–7");
    }

    #[test]
    fn snippets_become_marked_and_plain_spans() {
        let found = spans("the \u{2}tax\u{3} refund for \u{2}2025\u{3}");
        let shape: Vec<(&str, bool)> = found.iter().map(|s| (s.text.as_str(), s.marked)).collect();
        assert_eq!(
            shape,
            vec![
                ("the ", false),
                ("tax", true),
                (" refund for ", false),
                ("2025", true)
            ]
        );
        // Markup in a document stays text: it is never interpreted.
        let plain = spans("<img src=x onerror=alert(1)>");
        assert_eq!(plain[0].text, "<img src=x onerror=alert(1)>");
        assert!(!plain[0].marked);
    }

    fn problem(path: &str, reason: Reason, attempts: u32) -> Problem {
        Problem {
            path: path.to_string(),
            reason,
            attempts,
        }
    }

    #[test]
    fn skipped_and_failed_files_say_why() {
        let skipped = not_indexed(&problem("/docs/scan.pdf", Reason::NeedsOcr, 1));
        assert_eq!(skipped.name, "scan.pdf");
        assert!(!skipped.failed && !skipped.parked);
        assert!(skipped.reason.contains("no text layer"));
        let failed = not_indexed(&problem("/docs/bad.pdf", Reason::Crashed, 1));
        assert!(failed.failed && !failed.parked);
        assert!(not_indexed(&problem("/docs/bad.pdf", Reason::Crashed, 2)).parked);
    }
}
