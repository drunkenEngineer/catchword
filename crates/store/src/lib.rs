//! Catchword store: one SQLite file with files, passages, the keyword index
//! and the passage vectors.
//!
//! The index is derived data. Everything in it can be rebuilt from the user's
//! files, so a damaged index is replaced, never repaired by hand.

use std::path::Path;
use std::sync::Once;

use std::collections::HashMap;

use catchword_engine::fuse::{fuse, Found};
use catchword_engine::Passage;
use rusqlite::{params, Connection, OptionalExtension, Transaction};

/// Bumped whenever the tables change. An older app must refuse a newer index.
///
/// Version 2 added page numbers to passages. Version 3 added passage vectors
/// and records how passages were cut and which model made the vectors.
pub const SCHEMA_VERSION: i64 = 3;

/// Words found in more than this share of passages are left out of keyword
/// queries (ADR-20).
const COMMON_SHARE: f64 = 0.05;

pub struct Store {
    conn: Connection,
    was_reset: bool,
}

/// One matching passage, with the file it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub passage_id: i64,
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
    /// Passages that are searchable by meaning.
    pub vectors: i64,
}

/// Make sqlite-vec part of every SQLite connection this process opens from
/// now on. Done once, before the first connection.
fn enable_vector_search() {
    static ENABLED: Once = Once::new();
    ENABLED.call_once(|| {
        // SAFETY: sqlite3_vec_init is the extension's entry point and has the
        // signature SQLite expects; the cast only restores that signature,
        // which the sqlite-vec crate declares without arguments. This is the
        // registration sqlite-vec documents for rusqlite.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *const std::os::raw::c_char,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> std::os::raw::c_int,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        enable_vector_search();
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        enable_vector_search();
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
                "DROP TABLE IF EXISTS passage_vectors;
                 DROP TABLE IF EXISTS passages_fts;
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
        // How many passages hold each word, read from the keyword index. A
        // temporary view for this connection only: nothing is stored.
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS temp.passage_words
                 USING fts5vocab(main, passages_fts, 'row');",
        )?;
        Ok(Self { conn, was_reset })
    }

    /// True when the index was made by an older version and was cleared on
    /// opening. The next scan fills it again.
    pub fn was_reset(&self) -> bool {
        self.was_reset
    }

    /// Record how passages are cut, for example which tokenizer and sizes.
    /// Passages cut another way would not match their files' next scan, so
    /// a different value clears the index for a rebuild. Returns true when
    /// it was cleared.
    pub fn use_pipeline(&mut self, pipeline: &str) -> rusqlite::Result<bool> {
        let stored = meta_get(&self.conn, "pipeline")?;
        if stored.as_deref() == Some(pipeline) {
            return Ok(false);
        }
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM files", ())?;
        drop_orphans(&tx)?;
        meta_set(&tx, "pipeline", pipeline)?;
        tx.commit()?;
        Ok(stored.is_some())
    }

    /// Record which model makes the vectors. Vectors from different models
    /// cannot be compared, so a different model drops the stored vectors and
    /// every passage waits to be embedded again. Returns true when vectors
    /// were dropped.
    pub fn use_model(&mut self, model: &str, dimensions: usize) -> rusqlite::Result<bool> {
        let stored = meta_get(&self.conn, "model")?;
        let current = format!("{model} {dimensions}");
        if stored.as_deref() == Some(current.as_str()) {
            return Ok(false);
        }
        let tx = self.conn.transaction()?;
        tx.execute_batch(&format!(
            "DROP TABLE IF EXISTS passage_vectors;
             CREATE VIRTUAL TABLE passage_vectors USING vec0(embedding float[{dimensions}]);"
        ))?;
        meta_set(&tx, "model", &current)?;
        tx.commit()?;
        Ok(stored.is_some())
    }

    /// Up to `limit` passages that have no vector yet, as (id, text).
    pub fn passages_without_vectors(&self, limit: usize) -> rusqlite::Result<Vec<(i64, String)>> {
        self.require_model()?;
        let mut statement = self.conn.prepare(
            "SELECT id, text FROM passages
             WHERE id NOT IN (SELECT rowid FROM passage_vectors)
             ORDER BY id LIMIT ?1",
        )?;
        let rows =
            statement.query_map(params![limit as i64], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    /// Store passage vectors, all in one transaction. A vector whose passage
    /// has gone in the meantime is skipped.
    pub fn put_vectors(&mut self, vectors: &[(i64, Vec<f32>)]) -> rusqlite::Result<()> {
        let dimensions = self.require_model()?;
        let tx = self.conn.transaction()?;
        for (id, vector) in vectors {
            if vector.len() != dimensions {
                return Err(misuse(&format!(
                    "a vector of {} numbers, but the model makes {dimensions}",
                    vector.len()
                )));
            }
            let exists = tx
                .query_row("SELECT 1 FROM passages WHERE id = ?1", params![id], |_| {
                    Ok(())
                })
                .optional()?
                .is_some();
            if exists {
                tx.execute("DELETE FROM passage_vectors WHERE rowid = ?1", params![id])?;
                tx.execute(
                    "INSERT INTO passage_vectors(rowid, embedding) VALUES (?1, ?2)",
                    params![id, as_blob(vector)],
                )?;
            }
        }
        tx.commit()
    }

    /// The passages nearest to `query` in meaning, best first. Empty when no
    /// model has been chosen yet.
    pub fn search_vector(&self, query: &[f32], limit: usize) -> rusqlite::Result<Vec<Hit>> {
        let Some(dimensions) = self.model_dimensions()? else {
            return Ok(Vec::new());
        };
        if query.len() != dimensions {
            return Err(misuse("the query vector has the wrong size"));
        }
        let mut statement = self.conn.prepare(
            "WITH nearest AS (
                 SELECT rowid, distance FROM passage_vectors
                 WHERE embedding MATCH ?1 AND k = ?2)
             SELECT p.id,
                    (SELECT MIN(path) FROM files f WHERE f.hash = p.hash),
                    (SELECT COUNT(*) FROM files f WHERE f.hash = p.hash),
                    p.page, p.start_line, p.end_line, p.text, nearest.distance
             FROM nearest JOIN passages p ON p.id = nearest.rowid
             ORDER BY nearest.distance",
        )?;
        let hits = statement.query_map(params![as_blob(query), limit as i64], |row| {
            let distance: f64 = row.get(7)?;
            Ok(Hit {
                passage_id: row.get(0)?,
                path: row.get(1)?,
                copies: row.get(2)?,
                page: row.get(3)?,
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                snippet: opening_words(&row.get::<_, String>(6)?),
                // Vectors have unit length, so this is the cosine similarity.
                score: 1.0 - distance * distance / 2.0,
            })
        })?;
        hits.collect()
    }

    /// The model the stored vectors come from, if one was chosen. Search
    /// must embed the query with the same model.
    pub fn embedding_model(&self) -> rusqlite::Result<Option<String>> {
        Ok(meta_get(&self.conn, "model")?
            .and_then(|model| model.rsplit_once(' ').map(|(name, _)| name.to_string())))
    }

    /// The vector size of the chosen model, if one was chosen.
    fn model_dimensions(&self) -> rusqlite::Result<Option<usize>> {
        Ok(meta_get(&self.conn, "model")?.and_then(|model| model.rsplit(' ').next()?.parse().ok()))
    }

    fn require_model(&self) -> rusqlite::Result<usize> {
        self.model_dimensions()?
            .ok_or_else(|| misuse("no embedding model has been chosen for this index"))
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
        let old_hash: Option<String> = tx
            .query_row(
                "SELECT hash FROM files WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .optional()?;
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
        // Only the content this file pointed to before can have lost its last
        // file. Checking just that one keeps each write the same cost however
        // large the index grows; a scan of the whole index here made indexing
        // time grow with the square of the library size (ADR-20).
        if let Some(old_hash) = old_hash.filter(|old| old != hash) {
            drop_content_if_unused(&tx, &old_hash)?;
        }
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

    /// Search by words and, when a query vector is given, by meaning, and
    /// combine both lists by rank (ADR-5). Each kind of search contributes
    /// up to `candidates` results; the combined list is best first and says
    /// how each result was found. A result found by words keeps the
    /// highlighted snippet of the keyword search.
    pub fn search_combined(
        &self,
        words: &str,
        meaning: Option<&[f32]>,
        candidates: usize,
    ) -> rusqlite::Result<Vec<(Hit, Found)>> {
        let by_words = self.search_keyword(words, candidates)?;
        let by_meaning = match meaning {
            Some(vector) => self.search_vector(vector, candidates)?,
            None => Vec::new(),
        };
        let ids = |hits: &[Hit]| hits.iter().map(|hit| hit.passage_id).collect::<Vec<_>>();
        let order = fuse(&ids(&by_words), &ids(&by_meaning));
        let mut hits: HashMap<i64, Hit> = HashMap::new();
        for hit in by_meaning.into_iter().chain(by_words) {
            hits.insert(hit.passage_id, hit);
        }
        Ok(order
            .into_iter()
            .filter_map(|result| {
                let mut hit = hits.remove(&result.item)?;
                hit.score = result.score;
                Some((hit, result.found))
            })
            .collect())
    }

    /// The full text of one passage, for a preview or for judging a result.
    pub fn passage_text(&self, id: i64) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT text FROM passages WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
    }

    /// Keyword search: passages with any of the words, ranked by BM25, so
    /// passages with more of the words, and rarer ones, come first.
    ///
    /// Identical copies of a file share one content, so they appear once.
    pub fn search_keyword(&self, query: &str, limit: usize) -> rusqlite::Result<Vec<Hit>> {
        let words = self.uncommon(query_words(query))?;
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let fts_query = words
            .iter()
            .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR ");
        // A passage must hold at least half of the query's distinct words.
        // Below that, matches are mostly chance: an English question against
        // an Arabic passage shares only a year or a name (ADR-20).
        let needed = words.len().div_ceil(2);
        // highlight() marks each word the keyword index matched, between
        // two control characters that passage text never contains.
        let mut statement = self.conn.prepare(
            "SELECT p.id,
                    (SELECT MIN(path) FROM files f WHERE f.hash = p.hash),
                    (SELECT COUNT(*) FROM files f WHERE f.hash = p.hash),
                    p.page,
                    p.start_line,
                    p.end_line,
                    snippet(passages_fts, 0, '[', ']', ' ... ', 24),
                    bm25(passages_fts),
                    highlight(passages_fts, 0, char(2), char(3))
             FROM passages_fts
             JOIN passages p ON p.id = passages_fts.rowid
             WHERE passages_fts MATCH ?1
             ORDER BY bm25(passages_fts)
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![fts_query, (limit * 5 + 20) as i64], |row| {
            let hit = Hit {
                passage_id: row.get(0)?,
                path: row.get(1)?,
                copies: row.get(2)?,
                page: row.get(3)?,
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                snippet: row.get(6)?,
                // SQLite's bm25() is lower-is-better; flip it.
                score: -row.get::<_, f64>(7)?,
            };
            let marked: String = row.get(8)?;
            Ok((hit, matched_words(&marked)))
        })?;
        let mut hits = Vec::new();
        for row in rows {
            let (hit, matched) = row?;
            if matched >= needed {
                hits.push(hit);
                if hits.len() == limit {
                    break;
                }
            }
        }
        Ok(hits)
    }

    /// Leave out words found in more than COMMON_SHARE of passages, such as
    /// "the" or "de": they barely change the ranking, but scoring the
    /// passages they match made a keyword search ten times slower (ADR-20).
    /// If every word is that common, the rarest one is kept.
    fn uncommon(&self, words: Vec<String>) -> rusqlite::Result<Vec<String>> {
        let passages: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM passages", (), |row| row.get(0))?;
        let limit = (passages as f64 * COMMON_SHARE).max(1.0);
        let mut counted = Vec::new();
        for word in words {
            // The index keeps words in lower case. A word it splits further,
            // or stores without accents, is not found here and simply kept.
            let key: String = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            let holding: i64 = self
                .conn
                .query_row(
                    "SELECT doc FROM temp.passage_words WHERE term = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(0);
            counted.push((word, holding));
        }
        let rarest = counted.iter().map(|(_, holding)| *holding).min();
        let kept: Vec<String> = counted
            .iter()
            .filter(|(_, holding)| (*holding as f64) <= limit)
            .map(|(word, _)| word.clone())
            .collect();
        if !kept.is_empty() {
            return Ok(kept);
        }
        Ok(counted
            .into_iter()
            .filter(|(_, holding)| Some(*holding) == rarest)
            .map(|(word, _)| word)
            .take(1)
            .collect())
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
            vectors: if has_table(&self.conn, "passage_vectors")? {
                count("passage_vectors")?
            } else {
                0
            },
        })
    }
}

fn has_table(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![name],
        |row| row.get(0),
    )
}

fn meta_get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

fn meta_set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// An error for a call this store cannot honour.
fn misuse(message: &str) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
        Some(message.to_string()),
    )
}

/// A vector as sqlite-vec stores it: 4-byte little-endian floats.
fn as_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// The start of a passage, to show for a match found by meaning.
fn opening_words(text: &str) -> String {
    const WORDS: usize = 30;
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= WORDS {
        words.join(" ")
    } else {
        format!("{} ...", words[..WORDS].join(" "))
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

/// Remove one content, with its passages, their keyword entries and vectors,
/// if no file refers to it any more.
fn drop_content_if_unused(tx: &Transaction<'_>, hash: &str) -> rusqlite::Result<()> {
    let still_used = tx
        .query_row(
            "SELECT 1 FROM files WHERE hash = ?1 LIMIT 1",
            params![hash],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if still_used {
        return Ok(());
    }
    if has_table(tx, "passage_vectors")? {
        let ids: Vec<i64> = tx
            .prepare("SELECT id FROM passages WHERE hash = ?1")?
            .query_map(params![hash], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for id in ids {
            tx.execute("DELETE FROM passage_vectors WHERE rowid = ?1", params![id])?;
        }
    }
    // The keyword index is told about each deletion first, with the old text.
    tx.execute(
        "INSERT INTO passages_fts(passages_fts, rowid, text)
         SELECT 'delete', id, text FROM passages WHERE hash = ?1",
        params![hash],
    )?;
    tx.execute("DELETE FROM passages WHERE hash = ?1", params![hash])?;
    tx.execute("DELETE FROM contents WHERE hash = ?1", params![hash])?;
    Ok(())
}

/// Remove passages, their vectors and contents that no file refers to any more.
fn drop_orphans(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if has_table(tx, "passage_vectors")? {
        tx.execute(
            "DELETE FROM passage_vectors WHERE rowid IN (
                 SELECT id FROM passages WHERE hash NOT IN (SELECT hash FROM files))",
            (),
        )?;
    }
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

/// The words of a query that keyword search looks for: those with a letter
/// or digit, each once, whatever its case. Each becomes a quoted term, so
/// the user's text is never read as query syntax.
fn query_words(query: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in query.split_whitespace() {
        if word.chars().any(char::is_alphanumeric)
            && !words
                .iter()
                .any(|w| w.to_lowercase() == word.to_lowercase())
        {
            words.push(word.to_string());
        }
    }
    words
}

/// How many different words highlight() marked, between char(2) and char(3).
fn matched_words(marked: &str) -> usize {
    let mut found: Vec<String> = Vec::new();
    for piece in marked.split('\u{2}').skip(1) {
        let word = piece
            .split('\u{3}')
            .next()
            .unwrap_or_default()
            .to_lowercase();
        if !found.contains(&word) {
            found.push(word);
        }
    }
    found.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use catchword_engine::{chunk, chunk_pages, WordTokenizer};

    /// Build a store from (path, hash, text) triples.
    fn store_with(documents: &[(&str, &str, &str)]) -> Store {
        let mut store = Store::open_in_memory().unwrap();
        for (path, hash, text) in documents {
            store
                .put_file(
                    path,
                    text.len() as u64,
                    1,
                    hash,
                    &chunk(text, 50, 5, &WordTokenizer),
                )
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
            vectors: 0,
        };
        assert_eq!(store.counts().unwrap(), expected);
        let hits = store.search_keyword("refund", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].copies, 2);
    }

    /// `documents` plus 50 filler passages that all contain "the", so word
    /// counts look like a real library's, where few words are common.
    fn store_with_filler(documents: &[(&str, &str, &str)]) -> Store {
        let filler: Vec<(String, String, String)> = (0..50)
            .map(|n| {
                (
                    format!("/filler/{n}.txt"),
                    format!("filler{n}"),
                    format!("the filler number{n}"),
                )
            })
            .collect();
        let mut all: Vec<(&str, &str, &str)> = documents.to_vec();
        all.extend(
            filler
                .iter()
                .map(|(path, hash, text)| (path.as_str(), hash.as_str(), text.as_str())),
        );
        store_with(&all)
    }

    #[test]
    fn very_common_words_are_left_out_of_keyword_search() {
        let store = store_with_filler(&[
            ("/a.txt", "h1", "the notice of termination"),
            ("/b.txt", "h2", "the termination clause"),
        ]);
        // "the" is in every passage: it is left out, so only the two
        // passages about termination match, not all 52.
        let hits = store.search_keyword("the termination", 60).unwrap();
        let mut paths: Vec<&str> = hits.iter().map(|hit| hit.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["/a.txt", "/b.txt"]);
        // A query of common words only still searches, by its rarest word.
        assert!(!store.search_keyword("the", 10).unwrap().is_empty());
    }

    #[test]
    fn matched_words_are_counted_once_each() {
        let marked = "the \u{2}Rent\u{3} is due; \u{2}rent\u{3} and \u{2}notice\u{3} apply";
        assert_eq!(matched_words(marked), 2);
        assert_eq!(matched_words("nothing marked"), 0);
    }

    #[test]
    fn keyword_search_needs_half_the_words_and_survives_odd_input() {
        let store = store_with_filler(&[
            ("/a.txt", "h1", "notice of termination sent in March"),
            ("/b.txt", "h2", "termination clause, thirty days"),
        ]);
        assert_eq!(store.search_keyword("termination", 10).unwrap().len(), 2);
        // Two words: one is enough. Both passages match "termination"; the
        // one that also has "march" leads.
        let hits = store.search_keyword("termination march", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, "/a.txt");
        // A word found nowhere does not hide the passages that match.
        assert_eq!(
            store.search_keyword("termination zebra", 10).unwrap().len(),
            2
        );
        // Five words: a passage needs three. Only /a.txt has them.
        let hits = store
            .search_keyword("notice of termination in March", 10)
            .unwrap();
        let paths: Vec<&str> = hits.iter().map(|hit| hit.path.as_str()).collect();
        assert_eq!(paths, vec!["/a.txt"]);
        // The same word twice counts once.
        assert_eq!(
            store
                .search_keyword("Termination termination", 10)
                .unwrap()
                .len(),
            2
        );
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
            vectors: 0,
        };
        assert_eq!(store.counts().unwrap(), expected);
    }

    #[test]
    fn changed_content_replaces_the_old_text() {
        let mut store = store_with(&[("/a.txt", "h1", "first version")]);
        store
            .put_file(
                "/a.txt",
                14,
                2,
                "h2",
                &chunk("second version", 50, 5, &WordTokenizer),
            )
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
            .put_file(
                "/a.pdf",
                10,
                1,
                "h1",
                &chunk_pages(&pages, 50, 5, &WordTokenizer),
            )
            .unwrap();
        store
            .put_file(
                "/b.txt",
                10,
                1,
                "h2",
                &chunk("another refund note", 50, 5, &WordTokenizer),
            )
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
            .put_file(
                "/new.pdf",
                1,
                1,
                "h2",
                &chunk_pages(&pages, 50, 5, &WordTokenizer),
            )
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
            .execute_batch(&format!(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO meta VALUES ('schema_version', '{}');",
                SCHEMA_VERSION + 1
            ))
            .unwrap();
        let before = std::fs::read(&file).unwrap();

        let Err(error) = Store::open(&file) else {
            panic!("a newer index was opened");
        };
        assert!(error.to_string().contains("newer version"), "{error}");
        assert_eq!(std::fs::read(&file).unwrap(), before);
        std::fs::remove_file(&file).unwrap();
    }

    /// A unit-length vector pointing mostly along `axis`.
    fn toward(axis: usize) -> Vec<f32> {
        let mut vector = [0.1f32; 3];
        vector[axis] = 1.0;
        let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        vector.iter().map(|x| x / length).collect()
    }

    /// Three files, one passage each, with vectors along different axes.
    fn store_with_vectors() -> Store {
        let mut store = store_with(&[
            ("/rent.txt", "h1", "the rent is due monthly"),
            ("/tax.txt", "h2", "the tax refund was approved"),
            ("/cat.txt", "h3", "the cat sleeps"),
        ]);
        store.use_model("test-model", 3).unwrap();
        let missing = store.passages_without_vectors(10).unwrap();
        let vectors: Vec<(i64, Vec<f32>)> = missing
            .iter()
            .enumerate()
            .map(|(axis, (id, _))| (*id, toward(axis)))
            .collect();
        store.put_vectors(&vectors).unwrap();
        store
    }

    #[test]
    fn a_changed_file_drops_its_old_text_and_vectors_unless_shared() {
        let mut store = store_with_vectors();
        // A copy of rent.txt shares its content.
        store
            .put_file(
                "/copy.txt",
                1,
                1,
                "h1",
                &chunk("the rent is due monthly", 50, 5, &WordTokenizer),
            )
            .unwrap();
        // Both files change: tax.txt's old content is used by nothing else, so
        // it goes; rent.txt's old content is still used by the copy, so it stays.
        for path in ["/tax.txt", "/rent.txt"] {
            store
                .put_file(
                    path,
                    2,
                    2,
                    &format!("new {path}"),
                    &chunk("fresh text", 50, 5, &WordTokenizer),
                )
                .unwrap();
        }
        assert!(store.search_keyword("refund", 10).unwrap().is_empty());
        assert_eq!(
            store.search_keyword("monthly", 10).unwrap()[0].path,
            "/copy.txt"
        );
        let counts = store.counts().unwrap();
        // h1 (rent, kept for the copy), h3 (cat), and the two new contents.
        assert_eq!(counts.contents, 4);
        // The tax passage's vector went with it; rent's and cat's remain.
        assert_eq!(counts.vectors, 2);
    }

    #[test]
    fn vector_search_finds_the_nearest_passages_first() {
        let store = store_with_vectors();
        let hits = store.search_vector(&toward(1), 3).unwrap();
        let paths: Vec<&str> = hits.iter().map(|hit| hit.path.as_str()).collect();
        assert_eq!(paths[0], "/tax.txt");
        assert_eq!(hits.len(), 3);
        assert!(hits[0].score > 0.99, "{}", hits[0].score);
        assert!(hits[0].score > hits[1].score);
        assert_eq!(hits[0].snippet, "the tax refund was approved");
    }

    #[test]
    fn only_passages_without_vectors_are_listed() {
        let mut store = store_with(&[("/a.txt", "h1", "alpha"), ("/b.txt", "h2", "beta")]);
        store.use_model("test-model", 3).unwrap();
        let missing = store.passages_without_vectors(10).unwrap();
        assert_eq!(missing.len(), 2);
        store.put_vectors(&[(missing[0].0, toward(0))]).unwrap();
        let still = store.passages_without_vectors(10).unwrap();
        assert_eq!(still, vec![missing[1].clone()]);
        assert_eq!(store.counts().unwrap().vectors, 1);
    }

    #[test]
    fn purging_a_file_removes_its_vectors() {
        let mut store = store_with_vectors();
        store
            .purge_missing("/", &["/rent.txt".to_string(), "/cat.txt".to_string()])
            .unwrap();
        assert_eq!(store.counts().unwrap().vectors, 2);
        let hits = store.search_vector(&toward(1), 3).unwrap();
        assert!(hits.iter().all(|hit| hit.path != "/tax.txt"));
    }

    #[test]
    fn another_model_drops_the_old_vectors() {
        let mut store = store_with_vectors();
        assert!(!store.use_model("test-model", 3).unwrap());
        assert_eq!(store.counts().unwrap().vectors, 3);
        assert!(store.use_model("other-model", 4).unwrap());
        assert_eq!(store.counts().unwrap().vectors, 0);
        assert_eq!(store.passages_without_vectors(10).unwrap().len(), 3);
        // Vectors of the old size are now refused.
        let id = store.passages_without_vectors(1).unwrap()[0].0;
        assert!(store.put_vectors(&[(id, toward(0))]).is_err());
    }

    #[test]
    fn combined_search_ranks_results_found_both_ways_first() {
        let store = store_with_vectors();
        // "tax" matches one passage by words; the vector points at the same
        // passage, so it is found both ways and comes first.
        let results = store.search_combined("tax", Some(&toward(1)), 10).unwrap();
        assert_eq!(results[0].0.path, "/tax.txt");
        assert_eq!(results[0].1, Found::Both);
        assert!(results[0].0.snippet.contains("[tax]"));
        assert_eq!(results.len(), 3);
        assert!(results[1..]
            .iter()
            .all(|(_, found)| *found == Found::Meaning));
        // Without a query vector, only words count.
        let results = store.search_combined("rent", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Found::Keyword);
    }

    #[test]
    fn a_passage_can_be_read_back_by_its_id() {
        let store = store_with(&[("/a.txt", "h1", "the whole passage text")]);
        let id = store.search_keyword("whole", 1).unwrap()[0].passage_id;
        assert_eq!(
            store.passage_text(id).unwrap().as_deref(),
            Some("the whole passage text")
        );
        assert_eq!(store.passage_text(id + 1000).unwrap(), None);
    }

    #[test]
    fn the_index_remembers_its_model() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.embedding_model().unwrap(), None);
        store.use_model("model-a@1234", 3).unwrap();
        assert_eq!(
            store.embedding_model().unwrap().as_deref(),
            Some("model-a@1234")
        );
    }

    #[test]
    fn vectors_need_a_model_first() {
        let mut store = store_with(&[("/a.txt", "h1", "alpha")]);
        assert!(store.passages_without_vectors(10).is_err());
        assert!(store.put_vectors(&[(1, toward(0))]).is_err());
        assert!(store.search_vector(&toward(0), 3).unwrap().is_empty());
    }

    #[test]
    fn another_way_of_cutting_passages_clears_the_index() {
        let mut store = Store::open_in_memory().unwrap();
        assert!(!store.use_pipeline("words 200/30").unwrap());
        store
            .put_file("/a.txt", 5, 1, "h1", &chunk("alpha", 50, 5, &WordTokenizer))
            .unwrap();
        assert!(!store.use_pipeline("words 200/30").unwrap());
        assert_eq!(store.counts().unwrap().files, 1);
        assert!(store.use_pipeline("model tokens 350/50").unwrap());
        assert_eq!(store.counts().unwrap(), Counts::default());
    }
}
