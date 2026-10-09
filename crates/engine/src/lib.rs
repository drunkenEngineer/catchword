//! Catchword engine: scanning, reading and splitting documents.
//!
//! Privacy rule: this crate must never contain network code.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

mod attributes;
mod encoding;
pub mod exclude;
pub mod extract;
pub mod fuse;

pub use encoding::decode_text;
pub use exclude::Exclusions;
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
    /// Its content is not on this computer, so it must not be opened.
    pub cloud_only: bool,
}

/// What a scan found.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Scan {
    /// Every file, sorted by path.
    pub files: Vec<FileMeta>,
    /// Folders and files below the root that could not be read. What the
    /// index holds under them is unknown, not gone.
    pub unreadable: Vec<PathBuf>,
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

/// List the files under `root`, from the folder listings alone: no file is
/// opened.
///
/// Links are not followed, so a scan can never leave the folder the user chose.
/// Hidden and system files and folders are skipped, and so is whatever
/// `exclusions` leaves out. A folder that is skipped is not entered.
///
/// An error means the root itself cannot be read: an unplugged drive, a
/// moved folder or no access. Then nothing can be said about its files.
pub fn scan(root: &Path, exclusions: &Exclusions) -> io::Result<Scan> {
    fs::read_dir(root)?;
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !(hidden(e) || exclusions.excludes(e.path())));
    let mut found = Scan::default();
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                found.unreadable.extend(error.path().map(Path::to_path_buf));
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            found.unreadable.push(entry.into_path());
            continue;
        };
        let modified_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        found.files.push(FileMeta {
            path: entry.into_path(),
            size: meta.len(),
            modified_secs,
            cloud_only: attributes::cloud_only(&meta),
        });
    }
    found.files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

fn hidden(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    match entry.metadata() {
        Ok(meta) => attributes::hidden(&name, &meta),
        Err(_) => name.starts_with('.'),
    }
}

/// Read a plain-text or Markdown file and return its text with the hash of
/// its bytes. Files over `max_bytes` are refused.
///
/// The encoding is detected (see [`decode_text`]), and control characters
/// are dropped.
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
    let text = without_controls(&decode_text(&bytes));
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
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    // A piece at a time, so a large file is never held in memory whole.
    let mut piece = vec![0u8; 1 << 16];
    loop {
        match file.read(&mut piece) {
            Ok(0) => break,
            Ok(read) => hasher.update(&piece[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
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

/// The passage size the app uses, in the embedding model's tokens, and the
/// overlap between neighbouring passages. The benchmark found 200, 350 and
/// 500 within a point of each other; 350 stays for now (ADR-20).
pub const PASSAGE_TOKENS: usize = 350;
pub const PASSAGE_OVERLAP_TOKENS: usize = 50;

/// Splits text into an embedding model's tokens. Passages are sized in
/// tokens because a model silently ignores text beyond its limit.
pub trait Tokenizer {
    /// The byte offset in `text` where each token ends, in order, leaving
    /// out special tokens such as start and end markers.
    fn token_ends(&self, text: &str) -> Vec<usize>;
}

/// Counts each word as one token. Used in tests, and to size passages when
/// no embedding model is installed.
pub struct WordTokenizer;

impl Tokenizer for WordTokenizer {
    fn token_ends(&self, text: &str) -> Vec<usize> {
        text.split_whitespace()
            .map(|word| word.as_ptr() as usize - text.as_ptr() as usize + word.len())
            .collect()
    }
}

/// Words whose tokens are counted in one call to the tokenizer. A tokenizer
/// may stop counting at a limit of its own (Granite's is 32,768 tokens), so
/// a long text is counted a block at a time; between blocks it can also be
/// stopped (see [`chunk_while`]).
const BLOCK_WORDS: usize = 2_000;

/// Split text into passages of at most `max_tokens` tokens, cutting only
/// between words.
///
/// Neighbouring passages share up to `overlap_tokens` tokens of whole words,
/// so a sentence cut at a boundary is still whole in one of them. A single
/// word longer than `max_tokens` becomes a passage of its own.
pub fn chunk(
    text: &str,
    max_tokens: usize,
    overlap_tokens: usize,
    tokenizer: &dyn Tokenizer,
) -> Vec<Passage> {
    chunk_while(text, max_tokens, overlap_tokens, tokenizer, &mut || true).unwrap_or_default()
}

/// As [`chunk`], asking `keep_going` between blocks of words: None if it
/// said no, so a pause need not wait for a long file (IDX-5).
pub fn chunk_while(
    text: &str,
    max_tokens: usize,
    overlap_tokens: usize,
    tokenizer: &dyn Tokenizer,
    keep_going: &mut dyn FnMut() -> bool,
) -> Option<Vec<Passage>> {
    assert!(max_tokens > 0, "max_tokens must be positive");
    assert!(
        overlap_tokens < max_tokens,
        "overlap must be smaller than the passage"
    );

    // Asked every block of words from the very start: splitting a long
    // text takes time too.
    let mut words: Vec<(u32, &str)> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for word in line.split_whitespace() {
            if words.len() % BLOCK_WORDS == BLOCK_WORDS - 1 && !keep_going() {
                return None;
            }
            words.push((index as u32 + 1, word));
        }
    }
    if words.is_empty() {
        return Some(Vec::new());
    }

    // Passages are cut from the words joined by single spaces. Each token
    // is counted against the word its last byte falls in.
    let mut joined = String::new();
    let mut starts = Vec::with_capacity(words.len());
    for (count, (_, word)) in words.iter().enumerate() {
        if count % BLOCK_WORDS == BLOCK_WORDS - 1 && !keep_going() {
            return None;
        }
        if !joined.is_empty() {
            joined.push(' ');
        }
        starts.push(joined.len());
        joined.push_str(word);
    }
    let mut tokens = vec![0usize; words.len()];
    for first in (0..words.len()).step_by(BLOCK_WORDS) {
        if !keep_going() {
            return None;
        }
        let last = (first + BLOCK_WORDS).min(words.len()) - 1;
        let offset = starts[first];
        let block = &joined[offset..starts[last] + words[last].1.len()];
        for end in tokenizer.token_ends(block) {
            if end > 0 && end <= block.len() {
                tokens[starts.partition_point(|&start| start < offset + end) - 1] += 1;
            }
        }
    }

    let mut passages = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let mut end = start + 1;
        let mut used = tokens[start];
        while end < words.len() && used + tokens[end] <= max_tokens {
            used += tokens[end];
            end += 1;
        }
        let last = end - 1;
        passages.push(Passage {
            ordinal: passages.len() as u32,
            page: None,
            start_line: words[start].0,
            end_line: words[last].0,
            text: joined[starts[start]..starts[last] + words[last].1.len()].to_string(),
        });
        if end == words.len() {
            break;
        }
        // Step back over whole words worth at most `overlap_tokens`, but
        // always move forward.
        let mut next = end;
        let mut shared = 0;
        while next > start + 1 && shared + tokens[next - 1] <= overlap_tokens {
            shared += tokens[next - 1];
            next -= 1;
        }
        start = next;
    }
    Some(passages)
}

/// Split each page into passages on its own, so every passage lies on one
/// page and carries its number. Ordinals run on across pages.
pub fn chunk_pages(
    pages: &[String],
    max_tokens: usize,
    overlap_tokens: usize,
    tokenizer: &dyn Tokenizer,
) -> Vec<Passage> {
    chunk_pages_while(pages, max_tokens, overlap_tokens, tokenizer, &mut || true)
        .unwrap_or_default()
}

/// As [`chunk_pages`], asking `keep_going` as it goes: None if it said no.
pub fn chunk_pages_while(
    pages: &[String],
    max_tokens: usize,
    overlap_tokens: usize,
    tokenizer: &dyn Tokenizer,
    keep_going: &mut dyn FnMut() -> bool,
) -> Option<Vec<Passage>> {
    let mut passages = Vec::new();
    for (index, text) in pages.iter().enumerate() {
        for passage in chunk_while(text, max_tokens, overlap_tokens, tokenizer, keep_going)? {
            passages.push(Passage {
                ordinal: passages.len() as u32,
                page: Some(index as u32 + 1),
                ..passage
            });
        }
    }
    Some(passages)
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
        let passages = chunk(&text, 20, 5, &WordTokenizer);
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
        let passages = chunk("one two\n\nthree four\nfive", 3, 1, &WordTokenizer);
        assert_eq!((passages[0].start_line, passages[0].end_line), (1, 3));
        assert_eq!(passages[1].start_line, 3);
        assert_eq!(passages.last().unwrap().end_line, 4);
    }

    /// A stand-in for a real model's tokenizer: one token per three
    /// characters of each word, so long words cost more than short ones.
    struct Triples;

    impl Tokenizer for Triples {
        fn token_ends(&self, text: &str) -> Vec<usize> {
            let mut ends = Vec::new();
            for word in text.split_whitespace() {
                let start = word.as_ptr() as usize - text.as_ptr() as usize;
                let mut end = start;
                for (count, c) in word.chars().enumerate() {
                    end += c.len_utf8();
                    if count % 3 == 2 || end == start + word.len() {
                        ends.push(end);
                    }
                }
            }
            ends
        }
    }

    fn count_tokens(text: &str) -> usize {
        Triples.token_ends(text).len()
    }

    #[test]
    fn passages_fit_the_token_limit_and_cut_between_words() {
        let text = "a quick brown fox, extraordinarily well documented, jumps over \
                    the incomprehensibilities of lazy dogs and résumés"
            .repeat(3);
        let passages = chunk(&text, 12, 4, &Triples);
        assert!(passages.len() > 3);
        for passage in &passages {
            assert!(count_tokens(&passage.text) <= 12, "{passage:?}");
            // Every passage is made of whole words of the original text.
            for word in passage.text.split_whitespace() {
                assert!(text.split_whitespace().any(|w| w == word), "{word}");
            }
        }
        // Nothing is lost: the first and last words are in some passage.
        assert!(passages[0].text.starts_with("a quick"));
        assert!(passages.last().unwrap().text.ends_with("résumés"));
    }

    #[test]
    fn neighbouring_passages_share_whole_words() {
        let passages = chunk("one two three four five six seven", 3, 1, &WordTokenizer);
        let texts: Vec<&str> = passages.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["one two three", "three four five", "five six seven"]
        );
    }

    #[test]
    fn a_word_longer_than_the_limit_is_a_passage_of_its_own() {
        let long = "x".repeat(60); // 20 tokens with Triples
        let text = format!("before {long} after");
        let passages = chunk(&text, 5, 1, &Triples);
        let texts: Vec<&str> = passages.iter().map(|p| p.text.as_str()).collect();
        assert_eq!(texts, vec!["before", long.as_str(), "after"]);
    }

    #[test]
    fn empty_text_gives_no_passages() {
        assert!(chunk("  \n\n ", 10, 2, &WordTokenizer).is_empty());
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
        let found = scan(&resolve_folder(&folder).unwrap(), &Exclusions::default())
            .unwrap()
            .files;
        assert_eq!(found.len(), 1);
        assert!(found[0].path.as_os_str().len() > 260);
        assert_eq!(read_text(&found[0].path, 100).unwrap().0, "deep inside");

        // A folder whose own path is long.
        let found = scan(&resolve_folder(&deep).unwrap(), &Exclusions::default())
            .unwrap()
            .files;
        assert_eq!(found.len(), 1);
        assert_eq!(read_text(&found[0].path, 100).unwrap().0, "deep inside");
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_scan_leaves_out_excluded_folders_patterns_and_hidden_files() {
        let folder = test_folder("excluded");
        for sub in ["private", "work", ".git", "work/node_modules"] {
            fs::create_dir_all(folder.join(sub)).unwrap();
        }
        for file in [
            "private/diary.txt",
            "work/plan.md",
            "work/passwords.txt",
            "work/node_modules/readme.md",
            ".git/notes.txt",
            "kept.txt",
        ] {
            fs::write(folder.join(file), "text").unwrap();
        }
        let root = resolve_folder(&folder).unwrap();
        let mut exclusions = Exclusions::with_default_patterns();
        exclusions.folders.push(root.join("private"));
        let names = |found: Vec<FileMeta>| -> Vec<String> {
            found
                .iter()
                .map(|f| {
                    f.path
                        .strip_prefix(&root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .collect()
        };
        assert_eq!(
            names(scan(&root, &exclusions).unwrap().files),
            vec!["kept.txt", "work/plan.md"]
        );
        // Without exclusions only the dot folder is skipped.
        assert_eq!(scan(&root, &Exclusions::default()).unwrap().files.len(), 5);
        fs::remove_dir_all(&folder).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn files_hidden_by_windows_are_skipped() {
        let folder = test_folder("hidden-attribute");
        fs::write(folder.join("shown.txt"), "text").unwrap();
        fs::write(folder.join("hidden.txt"), "text").unwrap();
        let status = std::process::Command::new("attrib")
            .arg("+h")
            .arg(folder.join("hidden.txt"))
            .status()
            .unwrap();
        assert!(status.success());
        let found = scan(&resolve_folder(&folder).unwrap(), &Exclusions::default())
            .unwrap()
            .files;
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("shown.txt"));
        let _ = std::process::Command::new("attrib")
            .arg("-h")
            .arg(folder.join("hidden.txt"))
            .status();
        fs::remove_dir_all(&folder).unwrap();
    }

    /// SRC-6: a link inside a chosen folder that leads outside it is not
    /// followed, so its files are not indexed. Junctions need no special
    /// rights on Windows; symbolic links do (Developer Mode, or an
    /// administrator), so they are tried, and skipped where not allowed.
    #[test]
    fn links_and_junctions_out_of_a_chosen_folder_are_not_followed() {
        let folder = test_folder("links");
        let (chosen, outside) = (folder.join("chosen"), folder.join("outside"));
        fs::create_dir_all(&chosen).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(chosen.join("own.txt"), "text").unwrap();
        fs::write(outside.join("secret.txt"), "text").unwrap();

        let mut made = Vec::new();
        #[cfg(windows)]
        {
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(chosen.join("junction"))
                .arg(&outside)
                .stdout(std::process::Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "a junction needs no special rights");
            made.push("junction");
            use std::os::windows::fs::{symlink_dir, symlink_file};
            if symlink_dir(&outside, chosen.join("folder-link")).is_ok() {
                made.push("folder link");
            }
            if symlink_file(outside.join("secret.txt"), chosen.join("file-link.txt")).is_ok() {
                made.push("file link");
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&outside, chosen.join("folder-link")).unwrap();
            symlink(outside.join("secret.txt"), chosen.join("file-link.txt")).unwrap();
            made.extend(["folder link", "file link"]);
        }
        println!("made: {made:?}");

        // Each link is there, and leads to the file outside.
        for link in ["junction", "folder-link"] {
            if chosen.join(link).exists() {
                assert!(chosen.join(link).join("secret.txt").is_file());
            }
        }
        let found = scan(&resolve_folder(&chosen).unwrap(), &Exclusions::default()).unwrap();
        let names: Vec<_> = found
            .files
            .iter()
            .map(|f| f.path.file_name().unwrap())
            .collect();
        assert_eq!(names, ["own.txt"]);
        assert!(found.unreadable.is_empty(), "{:?}", found.unreadable);
        // Removing the folder removes the links, never what they lead to.
        fs::remove_dir_all(&chosen).unwrap();
        assert!(outside.join("secret.txt").is_file());
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_folder_that_cannot_be_read_is_an_error_not_an_empty_scan() {
        let gone = test_folder("unreachable").join("not-there");
        assert!(scan(&gone, &Exclusions::default()).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn cloud_only_files_are_found_without_being_opened() {
        let folder = test_folder("cloud-only");
        fs::write(folder.join("local.txt"), "text").unwrap();
        fs::write(folder.join("online.txt"), "text").unwrap();
        // The offline attribute is what an archived or cloud-only file has.
        let attrib = |flag: &str| {
            let status = std::process::Command::new("attrib")
                .arg(flag)
                .arg(folder.join("online.txt"))
                .status()
                .unwrap();
            assert!(status.success());
        };
        attrib("+o");
        let found = scan(&resolve_folder(&folder).unwrap(), &Exclusions::default()).unwrap();
        let cloud: Vec<bool> = found.files.iter().map(|f| f.cloud_only).collect();
        assert_eq!(cloud, vec![false, true]);
        attrib("-o");
        fs::remove_dir_all(&folder).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn a_sub_folder_that_cannot_be_read_is_reported() {
        let folder = test_folder("unreadable-sub");
        let locked = folder.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("inside.txt"), "text").unwrap();
        fs::write(folder.join("outside.txt"), "text").unwrap();
        // Deny this user the right to list the folder, then give it back.
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
        let found = scan(&resolve_folder(&folder).unwrap(), &Exclusions::default());
        icacls(&["/remove:d", &user]);
        let found = found.unwrap();
        assert_eq!(found.files.len(), 1);
        assert_eq!(found.unreadable.len(), 1);
        assert!(found.unreadable[0].ends_with("locked"));
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_text_file_in_a_legacy_encoding_is_read_as_its_text() {
        let folder = test_folder("legacy-encoding");
        let lease = "عقد الإيجار: مدة الإشعار ثلاثة أشهر، ويجب إرسال رسالة مسجلة \
                     قبل نهاية الربع. يرد المالك خلال خمسة عشر يوما.";
        let (bytes, _, _) = encoding_rs::WINDOWS_1256.encode(lease);
        let file = folder.join("lease.txt");
        fs::write(&file, &bytes).unwrap();
        let (text, hash) = read_text(&file, 1 << 20).unwrap();
        assert_eq!(text, lease);
        // The hash is of the bytes as they are on disk.
        assert_eq!(hash, content_hash(&bytes));
        fs::remove_dir_all(&folder).unwrap();
    }

    /// One token per word, like WordTokenizer, but it stops counting after
    /// 3,000, as a real tokenizer stops at its own limit.
    struct StopsCounting;

    impl Tokenizer for StopsCounting {
        fn token_ends(&self, text: &str) -> Vec<usize> {
            let mut ends = WordTokenizer.token_ends(text);
            ends.truncate(3_000);
            ends
        }
    }

    fn many_words(count: usize) -> String {
        (0..count)
            .map(|n| format!("word{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn every_word_of_a_long_text_is_counted_despite_the_tokenizer_limit() {
        let text = many_words(10_000);
        let passages = chunk(&text, 50, 10, &StopsCounting);
        let longest = passages
            .iter()
            .map(|p| p.text.split_whitespace().count())
            .max()
            .unwrap();
        assert_eq!(longest, 50);
        assert!(passages.last().unwrap().text.ends_with("word9999"));
    }

    #[test]
    fn cutting_a_long_text_can_be_stopped_between_blocks() {
        let text = many_words(5_000);
        let mut asked = 0;
        let stopped = chunk_while(&text, 50, 10, &WordTokenizer, &mut || {
            asked += 1;
            asked < 2
        });
        assert_eq!(stopped, None);
        assert_eq!(asked, 2);
        let pages = vec![text.clone(), text];
        let mut asked = 0;
        let stopped = chunk_pages_while(&pages, 50, 10, &WordTokenizer, &mut || {
            asked += 1;
            asked < 4
        });
        assert_eq!(stopped, None);
        let whole = chunk_while(&pages[0], 50, 10, &WordTokenizer, &mut || true).unwrap();
        assert_eq!(whole, chunk(&pages[0], 50, 10, &WordTokenizer));
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
        let passages = chunk_pages(&pages, 2, 1, &WordTokenizer);
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
        assert!(chunk("plain text", 5, 1, &WordTokenizer)
            .iter()
            .all(|p| p.page.is_none()));
    }
}
