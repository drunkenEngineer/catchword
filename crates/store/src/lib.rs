//! Catchword store: one SQLite file with files, passages and the keyword index.
//!
//! The index is derived data. Everything in it can be rebuilt from the user's
//! files, so a damaged index is replaced, never repaired by hand.

use std::path::Path;

use catchword_engine::Passage;
use rusqlite::{params, Connection, OptionalExtension, Transaction};

/// Bumped whenever the tables change. An older app must refuse a newer index.
///
/// Version 2 added page numbers to passages.
pub const SCHEMA_VERSION: i64 = 2;

pub struct Store {
    conn: Connection,
    was_reset: bool,
}

/// One matching passage, with the file it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub path: String,
    /// How many files share this exact content (1 means no copies).
    pub copies: i64,
    /// The page, for paged documents such as PDFs.
    pub page: Option<i64>,
    pub start_line: i64,
    pub end_line: i64,
    pub snippet: String,
    /// Higher is better.
    pub score: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    pub files: i64,
    pub contents: i64,
    pub passages: i64,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> rusqlite::Result<Self> {
        // The index is derived data: an older layout is cleared and refilled
        // by the next scan. A newer one is refused before anything is written.
        let found = stored_version(&conn)?;
        if let Some(found) = found.filter(|found| *found > SCHEMA_VERSION) {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                Some(format!(
                    "the index was made by a newer version of Catchword (layout {found}; \
                     this version reads {SCHEMA_VERSION}). Update Catchword, or delete \
                     the index file to rebuild it"
                )),
            ));
        }
        let was_reset = found.is_some_and(|found| found < SCHEMA_VERSION);

        conn.pragma_update(None, "journal_mode", "WAL")?;
        // Overwrite freed pages, so purged text cannot be read back from the file.
        conn.pragma_update(None, "secure_delete", "ON")?;
        if was_reset {
            let tx = conn.transaction()?;
            tx.execute_batch(
                "DROP TABLE IF EXISTS passages_fts;
                 DROP TABLE IF EXISTS passages;
                 DROP TABLE IF EXISTS files;
                 DROP TABLE IF EXISTS contents;
                 DROP TABLE IF EXISTS meta;",
            )?;
            tx.commit()?;
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS contents (hash TEXT PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS files (
                 path TEXT PRIMARY KEY,
                 size INTEGER NOT NULL,
                 modified_secs INTEGER NOT NULL,
                 hash TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS files_hash ON files(hash);
             CREATE TABLE IF NOT EXISTS passages (
                 id INTEGER PRIMARY KEY,
                 hash TEXT NOT NULL,
                 ordinal INTEGER NOT NULL,
                 page INTEGER,
                 start_line INTEGER NOT NULL,
                 end_line INTEGER NOT NULL,
                 text TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS passages_hash ON passages(hash);
             CREATE VIRTUAL TABLE IF NOT EXISTS passages_fts USING fts5(
                 text, content='passages', content_rowid='id',
                 tokenize='unicode61 remove_diacritics 2');",
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO meta(key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )?;
        Ok(Self { conn, was_reset })
    }

    /// True when the index was made by an older version and was cleared on
    /// opening. The next scan fills it again.
    pub fn was_reset(&self) -> bool {
        self.was_reset
    }

    /// True when size and modified time match what is stored, so the file
    /// does not need to be opened again.
    pub fn is_unchanged(
        &self,
        path: &str,
        size: u64,
        modified_secs: i64,
    ) -> rusqlite::Result<bool> {
        let stored: Option<(i64, i64)> = self
            .conn
            .query_row(
                "SELECT size, modified_secs FROM files WHERE path = ?1",
                params![path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(stored == Some((size as i64, modified_secs)))
    }

    pub fn has_content(&self, hash: &str) -> rusqlite::Result<bool> {
        let found = self
            .conn
            .query_row(
                "SELECT 1 FROM contents WHERE hash = ?1",
                params![hash],
                |_| Ok(()),
            )
            .optional()?;
        Ok(found.is_some())
    }

    /// Record a file and, when its content is new, the passages of that content.
    ///
    /// One transaction per document: either all of it is searchable or none.
    /// Returns true when the content was new.
    pub fn put_file(
        &mut self,
        path: &str,
        size: u64,
        modified_secs: i64,
        hash: &str,
        passages: &[Passage],
    ) -> rusqlite::Result<bool> {
        let tx = self.conn.transaction()?;
        let is_new = tx
            .query_row(
                "SELECT 1 FROM contents WHERE hash = ?1",
                params![hash],
                |_| Ok(()),
            )
            .optional()?
            .is_none();
        if is_new {
            tx.execute("INSERT INTO contents(hash) VALUES (?1)", params![hash])?;
            for passage in passages {
                tx.execute(
                    "INSERT INTO passages(hash, ordinal, page, start_line, end_line, text)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        hash,
                        passage.ordinal,
                        passage.page,
                        passage.start_line,
                        passage.end_line,
                        passage.text
                    ],
                )?;
                tx.execute(
                    "INSERT INTO passages_fts(rowid, text) VALUES (?1, ?2)",
                    params![tx.last_insert_rowid(), passage.text],
                )?;
            }
        }
        tx.execute(
            "INSERT INTO files(path, size, modified_secs, hash) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
                 size = excluded.size,
                 modified_secs = excluded.modified_secs,
                 hash = excluded.hash",
            params![path, size as i64, modified_secs, hash],
        )?;
        drop_orphans(&tx)?;
        tx.commit()?;
        Ok(is_new)
    }

    /// Forget files under `root_prefix` that the latest scan did not see, then
    /// purge any text no remaining file points to. Returns the files removed.
    ///
    /// Call this only when the folder was reachable: an unplugged drive is
    /// offline, not deleted.
    pub fn purge_missing(&mut self, root_prefix: &str, seen: &[String]) -> rusqlite::Result<usize> {
        let tx = self.conn.transaction()?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS seen (path TEXT PRIMARY KEY);
             DELETE FROM seen;",
        )?;
        {
            let mut insert = tx.prepare("INSERT OR IGNORE INTO seen(path) VALUES (?1)")?;
            for path in seen {
                insert.execute(params![path])?;
            }
        }
        let removed = tx.execute(
            "DELETE FROM files
             WHERE substr(path, 1, length(?1)) = ?1
               AND path NOT IN (SELECT path FROM seen)",
            params![root_prefix],
        )?;
        drop_orphans(&tx)?;
        tx.commit()?;
        Ok(removed)
    }

    /// Keyword search: every word must appear, ranked by BM25.
    ///
    /// Identical copies of a file share one content, so they appear once.
    pub fn search_keyword(&self, query: &str, limit: usize) -> rusqlite::Result<Vec<Hit>> {
        let Some(fts_query) = to_fts_query(query) else {
            return Ok(Vec::new());
        };
        let mut statement = self.conn.prepare(
            "SELECT (SELECT MIN(path) FROM files f WHERE f.hash = p.hash),
                    (SELECT COUNT(*) FROM files f WHERE f.hash = p.hash),
                    p.page,
                    p.start_line,
                    p.end_line,
                    snippet(passages_fts, 0, '[', ']', ' ... ', 24),
                    bm25(passages_fts)
             FROM passages_fts
             JOIN passages p ON p.id = passages_fts.rowid
             WHERE passages_fts MATCH ?1
             ORDER BY bm25(passages_fts)
             LIMIT ?2",
        )?;
        let hits = statement.query_map(params![fts_query, limit as i64], |row| {
            Ok(Hit {
                path: row.get(0)?,
                copies: row.get(1)?,
                page: row.get(2)?,
                start_line: row.get(3)?,
                end_line: row.get(4)?,
                snippet: row.get(5)?,
                // SQLite's bm25() is lower-is-better; flip it.
                score: -row.get::<_, f64>(6)?,
            })
        })?;
        hits.collect()
    }

    pub fn counts(&self) -> rusqlite::Result<Counts> {
        let count = |table: &str| -> rusqlite::Result<i64> {
            self.conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), (), |row| {
                    row.get(0)
                })
        };
        Ok(Counts {
            files: count("files")?,
            contents: count("contents")?,
            passages: count("passages")?,
        })
    }
}

/// The index layout version recorded in the file, or None for a new file.
fn stored_version(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    let has_meta: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta')",
        (),
        |row| row.get(0),
    )?;
    if !has_meta {
        return Ok(None);
    }
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            (),
            |row| row.get(0),
        )
        .optional()?;
    // A missing or unreadable version is treated as the oldest.
    Ok(Some(value.and_then(|v| v.parse().ok()).unwrap_or(0)))
}

/// Remove passages and contents that no file refers to any more.
fn drop_orphans(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    // The keyword index is told about each deletion first, with the old text.
    tx.execute(
        "INSERT INTO passages_fts(passages_fts, rowid, text)
         SELECT 'delete', id, text FROM passages
         WHERE hash NOT IN (SELECT hash FROM files)",
        (),
    )?;
    tx.execute(
        "DELETE FROM passages WHERE hash NOT IN (SELECT hash FROM files)",
        (),
    )?;
    tx.execute(
        "DELETE FROM contents WHERE hash NOT IN (SELECT hash FROM files)",
        (),
    )?;
    Ok(())
}

/// Turn free text into a safe keyword query: each word quoted, all required.
///
/// Quoting means the user's text is never read as query syntax.
fn to_fts_query(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use catchword_engine::{chunk, chunk_pages};

    /// Build a store from (path, hash, text) triples.
    fn store_with(documents: &[(&str, &str, &str)]) -> Store {
        let mut store = Store::open_in_memory().unwrap();
        for (path, hash, text) in documents {
            store
                .put_file(path, text.len() as u64, 1, hash, &chunk(text, 50, 5))
                .unwrap();
        }
        store
    }

    #[test]
    fn identical_content_is_stored_once() {
        let store = store_with(&[
            ("/a/one.txt", "h1", "the tax refund letter"),
            ("/b/copy.txt", "h1", "the tax refund letter"),
        ]);
        let expected = Counts {
            files: 2,
            contents: 1,
            passages: 1,
        };
        assert_eq!(store.counts().unwrap(), expected);
        let hits = store.search_keyword("refund", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].copies, 2);
    }

    #[test]
    fn search_requires_every_word_and_survives_odd_input() {
        let store = store_with(&[
            ("/a.txt", "h1", "notice of termination sent in March"),
            ("/b.txt", "h2", "termination clause, thirty days"),
        ]);
        assert_eq!(store.search_keyword("termination", 10).unwrap().len(), 2);
        let hits = store.search_keyword("termination march", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "/a.txt");
        assert!(store
            .search_keyword("\"unbalanced (quote* AND", 10)
            .unwrap()
            .is_empty());
        assert!(store.search_keyword("  -- ", 10).unwrap().is_empty());
    }

    #[test]
    fn purge_forgets_missing_files_and_their_text() {
        let mut store = store_with(&[
            ("/docs/a.txt", "h1", "alpha beta"),
            ("/docs/b.txt", "h2", "gamma delta"),
            ("/other/c.txt", "h3", "epsilon"),
        ]);
        let removed = store
            .purge_missing("/docs/", &["/docs/a.txt".to_string()])
            .unwrap();
        assert_eq!(removed, 1);
        assert!(store.search_keyword("gamma", 10).unwrap().is_empty());
        assert_eq!(store.search_keyword("epsilon", 10).unwrap().len(), 1);
        let expected = Counts {
            files: 2,
            contents: 2,
            passages: 2,
        };
        assert_eq!(store.counts().unwrap(), expected);
    }

    #[test]
    fn changed_content_replaces_the_old_text() {
        let mut store = store_with(&[("/a.txt", "h1", "first version")]);
        store
            .put_file("/a.txt", 14, 2, "h2", &chunk("second version", 50, 5))
            .unwrap();
        assert!(store.search_keyword("first", 10).unwrap().is_empty());
        assert_eq!(store.search_keyword("second", 10).unwrap().len(), 1);
        assert!(store.is_unchanged("/a.txt", 14, 2).unwrap());
        assert!(!store.is_unchanged("/a.txt", 14, 3).unwrap());
    }

    #[test]
    fn hits_carry_their_page() {
        let mut store = Store::open_in_memory().unwrap();
        let pages = ["cover".to_string(), "the refund decision".to_string()];
        store
            .put_file("/a.pdf", 10, 1, "h1", &chunk_pages(&pages, 50, 5))
            .unwrap();
        store
            .put_file("/b.txt", 10, 1, "h2", &chunk("another refund note", 50, 5))
            .unwrap();
        let mut found: Vec<(String, Option<i64>)> = store
            .search_keyword("refund", 10)
            .unwrap()
            .into_iter()
            .map(|hit| (hit.path, hit.page))
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec![
                ("/a.pdf".to_string(), Some(2)),
                ("/b.txt".to_string(), None)
            ]
        );
    }

    /// The layout of version 1, with one file in it.
    const VERSION_1: &str = "
        CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        INSERT INTO meta VALUES ('schema_version', '1');
        CREATE TABLE contents (hash TEXT PRIMARY KEY);
        CREATE TABLE files (path TEXT PRIMARY KEY, size INTEGER NOT NULL,
            modified_secs INTEGER NOT NULL, hash TEXT NOT NULL);
        CREATE TABLE passages (id INTEGER PRIMARY KEY, hash TEXT NOT NULL,
            ordinal INTEGER NOT NULL, start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL, text TEXT NOT NULL);
        CREATE VIRTUAL TABLE passages_fts USING fts5(text, content='passages',
            content_rowid='id', tokenize='unicode61 remove_diacritics 2');
        INSERT INTO contents VALUES ('h1');
        INSERT INTO files VALUES ('/old.txt', 8, 1, 'h1');
        INSERT INTO passages VALUES (1, 'h1', 0, 1, 1, 'old text');
        INSERT INTO passages_fts(rowid, text) VALUES (1, 'old text');";

    #[test]
    fn an_older_index_is_cleared_for_a_rebuild() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(VERSION_1).unwrap();
        let mut store = Store::init(conn).unwrap();
        assert!(store.was_reset());
        assert_eq!(store.counts().unwrap(), Counts::default());
        assert!(store.search_keyword("old", 10).unwrap().is_empty());

        let pages = ["fresh text".to_string()];
        store
            .put_file("/new.pdf", 1, 1, "h2", &chunk_pages(&pages, 50, 5))
            .unwrap();
        assert_eq!(store.search_keyword("fresh", 10).unwrap()[0].page, Some(1));
        assert_eq!(stored_version(&store.conn).unwrap(), Some(SCHEMA_VERSION));
        assert!(!Store::open_in_memory().unwrap().was_reset());
    }

    #[test]
    fn a_newer_index_is_refused_and_left_untouched() {
        let file = std::env::temp_dir().join("catchword-store-test-newer.db");
        let _ = std::fs::remove_file(&file);
        Connection::open(&file)
            .unwrap()
            .execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO meta VALUES ('schema_version', '3');",
            )
            .unwrap();
        let before = std::fs::read(&file).unwrap();

        let Err(error) = Store::open(&file) else {
            panic!("a newer index was opened");
        };
        assert!(error.to_string().contains("newer version"), "{error}");
        assert_eq!(std::fs::read(&file).unwrap(), before);
        std::fs::remove_file(&file).unwrap();
    }
}
