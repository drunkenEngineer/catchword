//! Catchword store: one SQLite file with files, passages, the keyword index
//! and the passage vectors.
//!
//! The index is derived data. Everything in it can be rebuilt from the user's
//! files, so a damaged index is replaced, never repaired by hand.

use std::path::Path;
use std::sync::Once;

use std::collections::HashMap;

use catchword_engine::extract::Reason;
use catchword_engine::fuse::{fuse, Found};
use catchword_engine::Passage;
/// The SQLite binding this crate's results and errors come from.
pub use rusqlite;
use rusqlite::{params, Connection, OptionalExtension, Transaction};

/// Bumped whenever the tables change. An older app must refuse a newer index.
///
/// Version 2 added page numbers to passages. Version 3 added passage vectors
/// and records how passages were cut and which model made the vectors.
/// Version 4 records the files that were not indexed, and why. Version 5
/// indexes file paths, so file and folder names can be searched.
pub const SCHEMA_VERSION: i64 = 5;

/// The oldest layout that is upgraded in place. Older ones are cleared and
/// refilled by the next scan: the index is derived data.
const OLDEST_UPGRADABLE: i64 = 3;

/// How the refusal of a newer index begins; see [`is_newer_layout`].
const NEWER_LAYOUT: &str = "the index was made by a newer version of Catchword";

/// True if opening failed because a newer version of Catchword made the
/// index. Such an index is never written to; it is not damaged either.
pub fn is_newer_layout(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(_, Some(message)) if message.starts_with(NEWER_LAYOUT))
}

/// Words found in more than this share of passages are left out of keyword
/// queries (ADR-20).
const COMMON_SHARE: f64 = 0.05;

/// Words in more than this share of file paths, such as the folders every
/// file sits in, say nothing about which file is meant.
const NAME_COMMON_SHARE: f64 = 0.5;

/// Results a name search contributes before they are combined.
const NAME_CANDIDATES: usize = 20;

/// The keyword indexes: the words of passages, and file paths.
const KEYWORD_INDEXES: [&str; 2] = ["passages_fts", "names_fts"];

/// A purge that removes more than this share of the passages deletes
/// plainly and then compacts the keyword indexes. Below it, deleting each
/// entry from them in place is quicker (ADR-22: about 4 ms a passage in
/// place, against compacting at about 0.06 ms a passage left).
const BULK_SHARE: f64 = 0.02;

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
    /// When the file was last changed, in seconds since 1970 (SEA-2).
    pub modified_secs: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    pub files: i64,
    pub contents: i64,
    pub passages: i64,
    /// Passages that are searchable by meaning.
    pub vectors: i64,
}

/// Which files a search may return (SEA-6). An empty list allows all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Folder paths: a file must be inside one of them.
    pub folders: Vec<String>,
    /// Name extensions, lower case and without the dot: a file must have one.
    pub extensions: Vec<String>,
    /// A file must have changed at or after this time, in seconds since 1970.
    pub modified_since: Option<i64>,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty() && self.extensions.is_empty() && self.modified_since.is_none()
    }

    /// True if a file at `path`, last changed at `modified_secs`, may be shown.
    fn admits(&self, path: &str, modified_secs: i64) -> bool {
        self.allows(path)
            && self
                .modified_since
                .is_none_or(|since| modified_secs >= since)
    }

    pub fn allows(&self, path: &str) -> bool {
        let inside = |folder: &String| {
            let folder = folder.trim_end_matches(['/', '\\']);
            path.strip_prefix(folder)
                .is_some_and(|rest| rest.starts_with(['/', '\\']))
        };
        let extension = Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase);
        (self.folders.is_empty() || self.folders.iter().any(inside))
            && (self.extensions.is_empty()
                || extension.is_some_and(|extension| self.extensions.contains(&extension)))
    }
}

/// A file that is not in the index, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub reason: Reason,
    /// How many times reading it was tried, for this size and date.
    pub attempts: u32,
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
                    *mut *mut std::os::raw::c_char,
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
                    "{NEWER_LAYOUT} (layout {found}; this version reads {SCHEMA_VERSION}). \
                     Update Catchword, or rebuild the index"
                )),
            ));
        }
        let was_reset = found.is_some_and(|found| found < OLDEST_UPGRADABLE);
        let needs_names = found.is_some_and(|found| found < 5) && !was_reset;

        conn.pragma_update(None, "journal_mode", "WAL")?;
        // Overwrite freed pages, so purged text cannot be read back from the file.
        conn.pragma_update(None, "secure_delete", "ON")?;
        if was_reset {
            let tx = conn.transaction()?;
            tx.execute_batch(
                "DROP TABLE IF EXISTS names_fts;
                 DROP TABLE IF EXISTS problems;
                 DROP TABLE IF EXISTS passage_vectors;
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
                 tokenize='unicode61 remove_diacritics 2');
             CREATE TABLE IF NOT EXISTS problems (
                 path TEXT PRIMARY KEY,
                 size INTEGER NOT NULL,
                 modified_secs INTEGER NOT NULL,
                 reason TEXT NOT NULL,
                 attempts INTEGER NOT NULL);
             CREATE VIRTUAL TABLE IF NOT EXISTS names_fts USING fts5(
                 path, content='files', content_rowid='rowid',
                 tokenize='unicode61 remove_diacritics 2');
             CREATE TRIGGER IF NOT EXISTS files_name_added AFTER INSERT ON files BEGIN
                 INSERT INTO names_fts(rowid, path) VALUES (new.rowid, new.path);
             END;
             CREATE TRIGGER IF NOT EXISTS files_name_removed AFTER DELETE ON files BEGIN
                 INSERT INTO names_fts(names_fts, rowid, path)
                     VALUES ('delete', old.rowid, old.path);
             END;
             CREATE TRIGGER IF NOT EXISTS files_name_changed AFTER UPDATE OF path ON files BEGIN
                 INSERT INTO names_fts(names_fts, rowid, path)
                     VALUES ('delete', old.rowid, old.path);
                 INSERT INTO names_fts(rowid, path) VALUES (new.rowid, new.path);
             END;",
        )?;
        if needs_names {
            // The files of an older index are not in the name index yet.
            conn.execute("INSERT INTO names_fts(names_fts) VALUES ('rebuild')", ())?;
        }
        // PRIV-5: an index made before deleted entries were removed at once,
        // or one whose bulk purge was cut short, is compacted now.
        if compact_keyword_indexes(&conn)? {
            checkpoint(&conn)?;
        }
        conn.execute(
            "INSERT OR IGNORE INTO meta(key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )?;
        // Layouts from OLDEST_UPGRADABLE on only lack tables, and the
        // statements above have just created them.
        meta_set(&conn, "schema_version", &SCHEMA_VERSION.to_string())?;
        // How many passages hold each word, read from the keyword index. A
        // temporary view for this connection only: nothing is stored.
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS temp.passage_words
                 USING fts5vocab(main, passages_fts, 'row');",
        )?;
        Ok(Self { conn, was_reset })
    }

    /// Record when a scan of every folder last finished, in seconds since
    /// 1970 (OBS-2).
    pub fn set_last_scan(&self, secs: i64) -> rusqlite::Result<()> {
        meta_set(&self.conn, "last_scan", &secs.to_string())
    }

    pub fn last_scan(&self) -> rusqlite::Result<Option<i64>> {
        Ok(meta_get(&self.conn, "last_scan")?.and_then(|secs| secs.parse().ok()))
    }

    /// A quick check of the file's structure, for every start: true if no
    /// damage was found.
    pub fn quick_check(&self) -> rusqlite::Result<bool> {
        let result: String = self
            .conn
            .query_row("PRAGMA quick_check", (), |row| row.get(0))?;
        Ok(result == "ok")
    }

    /// The full check, on demand: the file, then both keyword indexes
    /// against the tables they index. True if no damage was found.
    pub fn integrity_check(&self) -> rusqlite::Result<bool> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check", (), |row| row.get(0))?;
        if result != "ok" {
            return Ok(false);
        }
        for table in ["passages_fts", "names_fts"] {
            // A rank of 1 compares the index with the table it indexes, too.
            let check = format!("INSERT INTO {table}({table}, rank) VALUES ('integrity-check', 1)");
            match self.conn.execute(&check, ()) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(error, _))
                    if error.code == rusqlite::ErrorCode::DatabaseCorrupt =>
                {
                    return Ok(false)
                }
                Err(error) => return Err(error),
            }
        }
        Ok(true)
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
        begin_bulk_delete(&tx)?;
        tx.execute("DELETE FROM files", ())?;
        drop_orphans(&tx)?;
        meta_set(&tx, "pipeline", pipeline)?;
        tx.commit()?;
        compact_keyword_indexes(&self.conn)?;
        checkpoint(&self.conn)?;
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
                modified_secs: 0,
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
        tx.execute("DELETE FROM problems WHERE path = ?1", params![path])?;
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
        // The passages of the leaving files, copies elsewhere included: an
        // estimate is enough to choose how to delete.
        let (leaving, all): (i64, i64) = tx.query_row(
            "SELECT (SELECT COUNT(*) FROM passages WHERE hash IN (
                         SELECT hash FROM files
                         WHERE substr(path, 1, length(?1)) = ?1
                           AND path NOT IN (SELECT path FROM seen))),
                    (SELECT COUNT(*) FROM passages)",
            params![root_prefix],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if leaving as f64 > all as f64 * BULK_SHARE {
            begin_bulk_delete(&tx)?;
        }
        let removed = tx.execute(
            "DELETE FROM files
             WHERE substr(path, 1, length(?1)) = ?1
               AND path NOT IN (SELECT path FROM seen)",
            params![root_prefix],
        )?;
        tx.execute(
            "DELETE FROM problems
             WHERE substr(path, 1, length(?1)) = ?1
               AND path NOT IN (SELECT path FROM seen)",
            params![root_prefix],
        )?;
        drop_orphans(&tx)?;
        tx.commit()?;
        compact_keyword_indexes(&self.conn)?;
        checkpoint(&self.conn)?;
        Ok(removed)
    }

    /// Every path the index knows that starts with `prefix`: files in the
    /// index and files recorded as not indexed.
    pub fn known_paths(&self, prefix: &str) -> rusqlite::Result<Vec<String>> {
        let mut statement = self.conn.prepare(
            "SELECT path FROM files WHERE substr(path, 1, length(?1)) = ?1
             UNION
             SELECT path FROM problems WHERE substr(path, 1, length(?1)) = ?1",
        )?;
        let paths = statement.query_map(params![prefix], |row| row.get(0))?;
        paths.collect()
    }

    /// Search by words and, when a query vector is given, by meaning, and
    /// combine both lists by rank (ADR-5). Each kind of search contributes
    /// up to `candidates` results; the combined list is best first and says
    /// how each result was found. A result found by words keeps the
    /// highlighted snippet of the keyword search.
    ///
    /// Passages only: the evaluation measures this (ADR-19), and its test
    /// files are named after their content, so names would flatter it.
    pub fn search_combined(
        &self,
        words: &str,
        meaning: Option<&[f32]>,
        candidates: usize,
    ) -> rusqlite::Result<Vec<(Hit, Found)>> {
        self.combine(words, meaning, false, candidates, &Filter::default())
    }

    /// As `search_combined`, and files whose name or folder names hold the
    /// query's words too (SEA-1). The app searches this way.
    /// Results only from files `filter` allows (SEA-6).
    pub fn search_with_names(
        &self,
        words: &str,
        meaning: Option<&[f32]>,
        candidates: usize,
        filter: &Filter,
    ) -> rusqlite::Result<Vec<(Hit, Found)>> {
        self.combine(words, meaning, true, candidates, filter)
    }

    fn combine(
        &self,
        words: &str,
        meaning: Option<&[f32]>,
        names: bool,
        candidates: usize,
        filter: &Filter,
    ) -> rusqlite::Result<Vec<(Hit, Found)>> {
        // A filter leaves out some of the best candidates, so take more.
        let widen = if filter.is_empty() { 1 } else { 4 };
        let by_words = self.within(filter, self.search_keyword(words, candidates * widen)?)?;
        let by_meaning = match meaning {
            Some(vector) => self.within(filter, self.search_vector(vector, candidates * widen)?)?,
            None => Vec::new(),
        };
        let by_name = if names {
            self.within(filter, self.search_names(words, NAME_CANDIDATES * widen)?)?
        } else {
            Vec::new()
        };
        // A quoted phrase asks for those words as written: what meaning or a
        // name found counts only if its passage has them too.
        let (phrases, _) = parse_query(words);
        let by_meaning = self.holding(&phrases, by_meaning)?;
        let by_name = self.holding(&phrases, by_name)?;
        let ids = |hits: &[Hit]| hits.iter().map(|hit| hit.passage_id).collect::<Vec<_>>();
        let order = fuse(&ids(&by_words), &ids(&by_meaning), &ids(&by_name));
        let mut hits: HashMap<i64, Hit> = HashMap::new();
        for hit in by_meaning.into_iter().chain(by_words) {
            hits.insert(hit.passage_id, hit);
        }
        // Found by its content too: the passage stays, shown as the file
        // whose name matched, among identical copies.
        for hit in by_name {
            match hits.get_mut(&hit.passage_id) {
                Some(found) => found.path = hit.path,
                None => {
                    hits.insert(hit.passage_id, hit);
                }
            }
        }
        let mut results = Vec::new();
        for fused in order {
            let Some(mut hit) = hits.remove(&fused.item) else {
                continue;
            };
            hit.score = fused.score;
            hit.modified_secs = self.modified_secs(&hit.path)?;
            results.push((hit, fused.found));
        }
        Ok(results)
    }

    /// The hits from a file `filter` allows, each shown as such a file: of
    /// identical copies, the one inside the filter.
    fn within(&self, filter: &Filter, hits: Vec<Hit>) -> rusqlite::Result<Vec<Hit>> {
        if filter.is_empty() {
            return Ok(hits);
        }
        let mut copies = self.conn.prepare(
            "SELECT f.path FROM files f JOIN passages p ON p.hash = f.hash
             WHERE p.id = ?1 ORDER BY f.path",
        )?;
        let mut kept = Vec::new();
        for mut hit in hits {
            if !filter.admits(&hit.path, self.modified_secs(&hit.path)?) {
                let paths =
                    copies.query_map(params![hit.passage_id], |row| row.get::<_, String>(0))?;
                let mut inside = None;
                for path in paths {
                    let path = path?;
                    if filter.admits(&path, self.modified_secs(&path)?) {
                        inside = Some(path);
                        break;
                    }
                }
                match inside {
                    Some(path) => hit.path = path,
                    None => continue,
                }
            }
            kept.push(hit);
        }
        Ok(kept)
    }

    /// The hits whose passage holds every one of `phrases`.
    fn holding(&self, phrases: &[String], hits: Vec<Hit>) -> rusqlite::Result<Vec<Hit>> {
        if phrases.is_empty() {
            return Ok(hits);
        }
        let all = phrases
            .iter()
            .map(|p| fts_phrase(p))
            .collect::<Vec<_>>()
            .join(" AND ");
        let mut kept = Vec::new();
        for hit in hits {
            let has: bool = self.conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM passages_fts
                                WHERE passages_fts MATCH ?1 AND rowid = ?2)",
                params![all, hit.passage_id],
                |row| row.get(0),
            )?;
            if has {
                kept.push(hit);
            }
        }
        Ok(kept)
    }

    fn modified_secs(&self, path: &str) -> rusqlite::Result<i64> {
        Ok(self
            .conn
            .query_row(
                "SELECT modified_secs FROM files WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    /// Files whose path holds the query's words: their file and folder
    /// names. Each is shown by its first passage. Words in more than half of
    /// all paths, such as the folders every file sits in, are left out; of
    /// the others found in some path, a path must hold two, or the only one.
    pub fn search_names(&self, query: &str, limit: usize) -> rusqlite::Result<Vec<Hit>> {
        let files: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", (), |row| row.get(0))?;
        let quote = |word: &str| format!("\"{}\"", word.replace('"', "\"\""));
        let mut words = Vec::new();
        for word in query_words(query) {
            // Counted by the name index itself, which splits and folds words
            // as it does paths.
            let holding: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM names_fts WHERE names_fts MATCH ?1",
                params![quote(&word)],
                |row| row.get(0),
            )?;
            if holding > 0 && holding as f64 <= files as f64 * NAME_COMMON_SHARE {
                words.push(word);
            }
        }
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let needed = words.len().min(2).max(words.len().div_ceil(2));
        let fts_query = words
            .iter()
            .map(|word| quote(word))
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut statement = self.conn.prepare(
            "SELECT p.id, f.path,
                    (SELECT COUNT(*) FROM files c WHERE c.hash = f.hash),
                    p.page, p.start_line, p.end_line, p.text,
                    bm25(names_fts),
                    highlight(names_fts, 0, char(2), char(3)),
                    f.modified_secs
             FROM names_fts
             JOIN files f ON f.rowid = names_fts.rowid
             JOIN passages p ON p.id =
                 (SELECT id FROM passages WHERE hash = f.hash ORDER BY ordinal LIMIT 1)
             WHERE names_fts MATCH ?1
             ORDER BY bm25(names_fts)
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
                snippet: opening_words(&row.get::<_, String>(6)?),
                score: -row.get::<_, f64>(7)?,
                modified_secs: row.get(9)?,
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

    /// The problem recorded for this file, if its size and date are still
    /// the ones it had then. A changed file gets a fresh try.
    pub fn known_problem(
        &self,
        path: &str,
        size: u64,
        modified_secs: i64,
    ) -> rusqlite::Result<Option<Problem>> {
        let row: Option<(String, u32)> = self
            .conn
            .query_row(
                "SELECT reason, attempts FROM problems
                 WHERE path = ?1 AND size = ?2 AND modified_secs = ?3",
                params![path, size as i64, modified_secs],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.and_then(|(code, attempts)| {
            Some(Problem {
                path: path.to_string(),
                reason: Reason::from_code(&code)?,
                attempts,
            })
        }))
    }

    /// Record why a file was not indexed. Returns how many times it has now
    /// been tried with this size and date.
    pub fn record_problem(
        &mut self,
        path: &str,
        size: u64,
        modified_secs: i64,
        reason: Reason,
    ) -> rusqlite::Result<u32> {
        let attempts = self
            .known_problem(path, size, modified_secs)?
            .map_or(1, |problem| problem.attempts + 1);
        self.conn.execute(
            "INSERT INTO problems(path, size, modified_secs, reason, attempts)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
                 size = excluded.size,
                 modified_secs = excluded.modified_secs,
                 reason = excluded.reason,
                 attempts = excluded.attempts",
            params![path, size as i64, modified_secs, reason.code(), attempts],
        )?;
        Ok(attempts)
    }

    /// Every file that is not in the index, by path (COV-2).
    pub fn problems(&self) -> rusqlite::Result<Vec<Problem>> {
        let mut statement = self
            .conn
            .prepare("SELECT path, reason, attempts FROM problems ORDER BY path")?;
        let rows = statement.query_map((), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
            ))
        })?;
        let mut problems = Vec::new();
        for row in rows {
            let (path, code, attempts) = row?;
            // A reason from a newer version is left out, not misread.
            if let Some(reason) = Reason::from_code(&code) {
                problems.push(Problem {
                    path,
                    reason,
                    attempts,
                });
            }
        }
        Ok(problems)
    }

    /// Forget the files whose reading failed, so the next run tries them
    /// again: the user's retry (COV-2). Returns how many.
    pub fn forget_failures(&mut self) -> rusqlite::Result<usize> {
        let mut forgotten = 0;
        for reason in Reason::ALL.into_iter().filter(|reason| reason.is_failure()) {
            forgotten += self.forget(reason)?;
        }
        Ok(forgotten)
    }

    /// Forget the files not indexed for `reason`, so the next run reads
    /// them again; for example the too-large ones after the limit is raised.
    pub fn forget(&mut self, reason: Reason) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM problems WHERE reason = ?1",
            params![reason.code()],
        )
    }

    /// The file a passage comes from: the first by path, if copies share it.
    /// The interface sends passage ids; the path comes from here, never
    /// from the interface (threat T15).
    pub fn file_of_passage(&self, id: i64) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT MIN(f.path) FROM passages p JOIN files f ON f.hash = p.hash
                 WHERE p.id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map(Option::flatten)
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
        let (phrases, words) = parse_query(query);
        let words = self.uncommon(words)?;
        if phrases.is_empty() && words.is_empty() {
            return Ok(Vec::new());
        }
        let any = phrases
            .iter()
            .chain(&words)
            .map(|text| fts_phrase(text))
            .collect::<Vec<_>>()
            .join(" OR ");
        // Each quoted phrase must be there, as written (SEA-5); all words,
        // phrases included, count for the ranking.
        let fts_query = if phrases.is_empty() {
            any
        } else {
            let required: Vec<String> = phrases.iter().map(|p| fts_phrase(p)).collect();
            format!("{} AND ({any})", required.join(" AND "))
        };
        // Without a phrase, a passage must hold at least half of the
        // query's distinct words. Below that, matches are mostly chance: an
        // English question against an Arabic passage shares only a year or a
        // name (ADR-20). A phrase is a sharper test of its own.
        let needed = if phrases.is_empty() {
            words.len().div_ceil(2)
        } else {
            0
        };
        // highlight() marks each word the keyword index matched, between
        // two control characters that passage text never contains.
        let mut statement = self.conn.prepare(
            "SELECT p.id,
                    (SELECT MIN(path) FROM files f WHERE f.hash = p.hash),
                    (SELECT COUNT(*) FROM files f WHERE f.hash = p.hash),
                    p.page,
                    p.start_line,
                    p.end_line,
                    snippet(passages_fts, 0, char(2), char(3), ' ... ', 24),
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
                modified_secs: 0,
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

/// For the rest of this transaction, deletions leave entries in the keyword
/// indexes, to be dropped by [`compact_keyword_indexes`] after it commits.
/// That is quicker for a large purge. If the app stops in between, the next
/// start compacts (see `Store::init`).
fn begin_bulk_delete(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    for table in KEYWORD_INDEXES {
        tx.execute(
            &format!("INSERT INTO {table}({table}, rank) VALUES ('secure-delete', 0)"),
            (),
        )?;
    }
    Ok(())
}

/// PRIV-5. The keyword indexes keep the entries of deleted passages and
/// files, readable in the file, until their segments happen to merge, unless
/// their "secure-delete" option is on. Where it is off, compact the index,
/// which drops every such entry, and switch it on: both in one transaction.
/// Returns true if anything was compacted.
fn compact_keyword_indexes(conn: &Connection) -> rusqlite::Result<bool> {
    let mut compacted = false;
    for table in KEYWORD_INDEXES {
        let on: Option<i64> = conn
            .query_row(
                &format!("SELECT v FROM {table}_config WHERE k = 'secure-delete'"),
                (),
                |row| row.get(0),
            )
            .optional()?;
        if on == Some(1) {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            &format!("INSERT INTO {table}({table}) VALUES ('optimize')"),
            (),
        )?;
        tx.execute(
            &format!("INSERT INTO {table}({table}, rank) VALUES ('secure-delete', 1)"),
            (),
        )?;
        tx.commit()?;
        compacted = true;
    }
    Ok(compacted)
}

/// Copy what the log holds into the index file, where freed pages are
/// overwritten, then empty the log, which still holds pages as they were
/// (PRIV-5). If a search is reading at that moment, the log is emptied at a
/// later checkpoint, or when the app closes.
fn checkpoint(conn: &Connection) -> rusqlite::Result<()> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", (), |_| Ok(()))
}

/// The words of a query that keyword search looks for: those with a letter
/// or digit, each once, whatever its case. Each becomes a quoted term, so
/// the user's text is never read as query syntax.
fn query_words(query: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in without_controls(query).split_whitespace() {
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

/// A query's quoted phrases, which must appear as written (SEA-5), and its
/// other words. Straight and curly quotes both count; a quote left open is
/// ignored, and so is a phrase with no letters or digits.
fn parse_query(query: &str) -> (Vec<String>, Vec<String>) {
    let straight = without_controls(query).replace(['\u{201c}', '\u{201d}', '\u{201e}'], "\"");
    let parts: Vec<&str> = straight.split('"').collect();
    // With every quote closed there is an odd number of parts, and every
    // other part, from the second, is inside quotes.
    let all_closed = parts.len() % 2 == 1;
    let mut phrases = Vec::new();
    let mut rest = String::new();
    for (index, part) in parts.iter().enumerate() {
        let inside = index % 2 == 1 && (all_closed || index + 1 < parts.len());
        if inside && part.chars().any(char::is_alphanumeric) {
            phrases.push(part.split_whitespace().collect::<Vec<_>>().join(" "));
        } else {
            rest.push(' ');
            rest.push_str(part);
        }
    }
    (phrases, query_words(&rest))
}

/// The query with control characters turned into spaces. The keyword
/// index reads its query as C text, so a NUL would end it early and leave
/// a quote open: a pasted document can carry one.
fn without_controls(query: &str) -> String {
    query
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Text as an FTS5 phrase: its words, in order, side by side.
fn fts_phrase(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
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
    fn quoted_phrases_are_found_apart_from_the_other_words() {
        assert_eq!(
            parse_query("\"notice period\" lease"),
            (vec!["notice period".to_string()], vec!["lease".to_string()])
        );
        assert_eq!(
            parse_query("\u{201c}three  months\u{201d} and \"rent\""),
            (
                vec!["three months".to_string(), "rent".to_string()],
                vec!["and".to_string()]
            )
        );
        // An open quote is ignored; so is a phrase of punctuation.
        assert_eq!(
            parse_query("\"notice period"),
            (vec![], vec!["notice".to_string(), "period".to_string()])
        );
        assert_eq!(parse_query("\"--\" rent").0, Vec::<String>::new());
    }

    #[test]
    fn a_quoted_phrase_must_appear_as_written() {
        let store = store_with_filler(&[
            ("/a.txt", "h1", "the notice period is three months"),
            ("/b.txt", "h2", "the period of notice is long"),
        ]);
        assert_eq!(store.search_keyword("notice period", 10).unwrap().len(), 2);
        let exact = store.search_keyword("\"notice period\"", 10).unwrap();
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].path, "/a.txt");
        // Other words only rank: they are not required.
        assert_eq!(
            store
                .search_keyword("\"notice period\" holiday", 10)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn with_a_phrase_meaning_counts_only_where_the_phrase_is() {
        let store = store_with_vectors();
        // The vector points at the rent passage; the phrase is in the tax one.
        let found = store
            .search_combined("\"tax refund\"", Some(&toward(0)), 10)
            .unwrap();
        let paths: Vec<&str> = found.iter().map(|(hit, _)| hit.path.as_str()).collect();
        assert_eq!(paths, vec!["/tax.txt"]);
        // Without quotes, meaning finds the rent passage too.
        assert!(store
            .search_combined("tax refund", Some(&toward(0)), 10)
            .unwrap()
            .iter()
            .any(|(hit, _)| hit.path == "/rent.txt"));
    }

    #[test]
    fn a_filter_keeps_files_of_its_folders_and_kinds() {
        let filter = Filter {
            folders: vec!["C:\\docs\\tax".into()],
            extensions: vec!["pdf".into()],
            ..Filter::default()
        };
        assert!(filter.allows("C:\\docs\\tax\\2025\\refund.PDF"));
        assert!(!filter.allows("C:\\docs\\tax\\notes.txt"));
        assert!(!filter.allows("C:\\docs\\taxes\\refund.pdf"));
        assert!(Filter::default().allows("/anything.txt"));
    }

    #[test]
    fn a_filtered_search_shows_the_copy_inside_the_folder() {
        let store = store_with_filler(&[
            ("/a/lease.txt", "h1", "the notice period is three months"),
            (
                "/b/lease-copy.txt",
                "h1",
                "the notice period is three months",
            ),
            (
                "/b/rent.md",
                "h2",
                "the rent is due monthly, notice the period",
            ),
        ]);
        let paths = |filter: &Filter| -> Vec<String> {
            let mut found: Vec<String> = store
                .search_with_names("notice period", None, 10, filter)
                .unwrap()
                .into_iter()
                .map(|(hit, _)| hit.path)
                .collect();
            found.sort();
            found
        };
        assert_eq!(
            paths(&Filter::default()),
            vec!["/a/lease.txt", "/b/rent.md"]
        );
        let in_b = Filter {
            folders: vec!["/b".into()],
            extensions: vec![],
            ..Filter::default()
        };
        assert_eq!(paths(&in_b), vec!["/b/lease-copy.txt", "/b/rent.md"]);
        let markdown = Filter {
            folders: vec![],
            extensions: vec!["md".into()],
            ..Filter::default()
        };
        assert_eq!(paths(&markdown), vec!["/b/rent.md"]);
    }

    /// Queries a person, or a pasted document, could type: quotes, keyword
    /// operators, brackets, control characters, other scripts. None may
    /// make a search fail (REL-2); the keyword index's own query language
    /// must never leak through.
    #[test]
    fn no_query_makes_a_search_fail() {
        const PIECES: [&str; 26] = [
            "lease",
            "notice",
            "\"",
            "\u{201c}",
            "\u{201d}",
            "*",
            "(",
            ")",
            "AND",
            "OR",
            "NOT",
            "NEAR",
            "NEAR(",
            ":",
            "^",
            "-",
            "+",
            "\0",
            "\u{1b}",
            "\t",
            " ",
            "\u{0639}\u{0642}\u{062f}",
            "e\u{301}",
            "\u{1f600}",
            "\\",
            "'",
        ];
        let store = store_with_filler(&[
            (
                "/docs/lease.txt",
                "h1",
                "the notice period of the lease is three months",
            ),
            (
                "/docs/NEAR-AND.txt",
                "h2",
                "a file whose name holds query words",
            ),
        ]);
        let filter = Filter {
            folders: vec!["/docs".into()],
            extensions: vec!["txt".into()],
            ..Filter::default()
        };
        let mut state: u64 = 5;
        for _ in 0..3_000 {
            let mut query = String::new();
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            for _ in 0..(state >> 33) % 8 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                query.push_str(PIECES[(state >> 33) as usize % PIECES.len()]);
                if state.is_multiple_of(3) {
                    query.push(' ');
                }
            }
            for result in [
                store.search_keyword(&query, 10).map(|_| ()),
                store.search_names(&query, 10).map(|_| ()),
                store
                    .search_with_names(&query, None, 10, &Filter::default())
                    .map(|_| ()),
                store
                    .search_with_names(&query, None, 10, &filter)
                    .map(|_| ()),
            ] {
                assert!(result.is_ok(), "{query:?}: {result:?}");
            }
        }
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
        assert!(results[0].0.snippet.contains("\u{2}tax\u{3}"));
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
    fn a_passage_knows_its_file() {
        let store = store_with(&[
            ("/b/copy.txt", "h1", "shared words"),
            ("/a/original.txt", "h1", "shared words"),
        ]);
        let id = store.search_keyword("shared", 1).unwrap()[0].passage_id;
        assert_eq!(
            store.file_of_passage(id).unwrap().as_deref(),
            Some("/a/original.txt")
        );
        assert_eq!(store.file_of_passage(id + 1000).unwrap(), None);
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
    fn problems_are_counted_per_size_and_date() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(
            store
                .record_problem("/a.pdf", 10, 1, Reason::Crashed)
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .record_problem("/a.pdf", 10, 1, Reason::Crashed)
                .unwrap(),
            2
        );
        let known = store.known_problem("/a.pdf", 10, 1).unwrap().unwrap();
        assert_eq!((known.reason, known.attempts), (Reason::Crashed, 2));
        // A changed file is a new file: no known problem, and counting restarts.
        assert_eq!(store.known_problem("/a.pdf", 11, 2).unwrap(), None);
        assert_eq!(
            store
                .record_problem("/a.pdf", 11, 2, Reason::Damaged)
                .unwrap(),
            1
        );
        store
            .record_problem("/b.pdf", 5, 1, Reason::NeedsOcr)
            .unwrap();
        let listed: Vec<(String, Reason, u32)> = store
            .problems()
            .unwrap()
            .into_iter()
            .map(|p| (p.path, p.reason, p.attempts))
            .collect();
        assert_eq!(
            listed,
            vec![
                ("/a.pdf".to_string(), Reason::Damaged, 1),
                ("/b.pdf".to_string(), Reason::NeedsOcr, 1)
            ]
        );
    }

    #[test]
    fn a_file_indexed_at_last_is_no_longer_a_problem() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_problem("/a.txt", 5, 1, Reason::CannotOpen)
            .unwrap();
        store
            .put_file(
                "/a.txt",
                5,
                1,
                "h1",
                &chunk("now readable", 50, 5, &WordTokenizer),
            )
            .unwrap();
        assert!(store.problems().unwrap().is_empty());
    }

    #[test]
    fn problems_of_files_that_are_gone_are_forgotten() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_problem("/docs/gone.pdf", 5, 1, Reason::NeedsOcr)
            .unwrap();
        store
            .record_problem("/docs/kept.pdf", 5, 1, Reason::NeedsOcr)
            .unwrap();
        store
            .record_problem("/other/x.pdf", 5, 1, Reason::NeedsOcr)
            .unwrap();
        store
            .purge_missing("/docs/", &["/docs/kept.pdf".to_string()])
            .unwrap();
        let paths: Vec<String> = store
            .problems()
            .unwrap()
            .into_iter()
            .map(|p| p.path)
            .collect();
        assert_eq!(paths, vec!["/docs/kept.pdf", "/other/x.pdf"]);
    }

    #[test]
    fn retrying_forgets_failures_but_not_rules() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_problem("/crash.pdf", 5, 1, Reason::Crashed)
            .unwrap();
        store
            .record_problem("/slow.pdf", 5, 1, Reason::TimedOut)
            .unwrap();
        store
            .record_problem("/scan.pdf", 5, 1, Reason::NeedsOcr)
            .unwrap();
        store
            .record_problem("/locked.pdf", 5, 1, Reason::Encrypted)
            .unwrap();
        assert_eq!(store.forget_failures().unwrap(), 2);
        let left: Vec<Reason> = store
            .problems()
            .unwrap()
            .into_iter()
            .map(|p| p.reason)
            .collect();
        assert_eq!(left, vec![Reason::Encrypted, Reason::NeedsOcr]);
    }

    #[test]
    fn problems_of_one_reason_can_be_forgotten() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_problem("/big.pdf", 5, 1, Reason::TooLarge)
            .unwrap();
        store
            .record_problem("/scan.pdf", 5, 1, Reason::NeedsOcr)
            .unwrap();
        assert_eq!(store.forget(Reason::TooLarge).unwrap(), 1);
        let left: Vec<Reason> = store
            .problems()
            .unwrap()
            .into_iter()
            .map(|p| p.reason)
            .collect();
        assert_eq!(left, vec![Reason::NeedsOcr]);
    }

    #[test]
    fn a_version_3_index_is_upgraded_in_place() {
        let file = std::env::temp_dir().join("catchword-store-test-upgrade.db");
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
        {
            let mut store = Store::open(&file).unwrap();
            store
                .put_file(
                    "/kept.txt",
                    4,
                    1,
                    "h1",
                    &chunk("kept text", 50, 5, &WordTokenizer),
                )
                .unwrap();
            // Make it look like version 3, which had no problems table.
            store
                .conn
                .execute_batch(
                    "DROP TABLE problems;
                     UPDATE meta SET value = '3' WHERE key = 'schema_version';",
                )
                .unwrap();
        }
        let mut store = Store::open(&file).unwrap();
        assert!(!store.was_reset());
        assert_eq!(store.counts().unwrap().files, 1);
        assert_eq!(store.search_keyword("kept", 5).unwrap().len(), 1);
        assert_eq!(stored_version(&store.conn).unwrap(), Some(SCHEMA_VERSION));
        store
            .record_problem("/new.pdf", 1, 1, Reason::NeedsOcr)
            .unwrap();
        assert_eq!(store.problems().unwrap().len(), 1);
    }

    #[test]
    fn known_paths_cover_files_and_problems_under_a_prefix() {
        let mut store = Store::open_in_memory().unwrap();
        let passages = chunk("some text", 50, 5, &WordTokenizer);
        store
            .put_file("/docs/a/one.txt", 1, 1, "h1", &passages)
            .unwrap();
        store
            .put_file("/docs/b/two.txt", 1, 1, "h2", &passages)
            .unwrap();
        store
            .record_problem("/docs/a/scan.pdf", 1, 1, Reason::NeedsOcr)
            .unwrap();
        let mut paths = store.known_paths("/docs/a/").unwrap();
        paths.sort();
        assert_eq!(paths, vec!["/docs/a/one.txt", "/docs/a/scan.pdf"]);
    }

    /// Files with their own content each, under one folder.
    fn named(paths: &[&str]) -> Store {
        let mut store = Store::open_in_memory().unwrap();
        for (n, path) in paths.iter().enumerate() {
            let text = format!("passage number {n} of some ordinary text");
            let passages = chunk(&text, 50, 5, &WordTokenizer);
            store
                .put_file(
                    path,
                    1,
                    1_700_000_000 + n as i64,
                    &format!("h{n}"),
                    &passages,
                )
                .unwrap();
        }
        store
    }

    fn found_names(store: &Store, query: &str) -> Vec<String> {
        store
            .search_names(query, 10)
            .unwrap()
            .into_iter()
            .map(|hit| hit.path)
            .collect()
    }

    #[test]
    fn file_and_folder_names_are_searched() {
        let store = named(&[
            "/home/ana/docs/Taxes 2023/invoice-march.pdf",
            "/home/ana/docs/Taxes 2023/invoice-april.pdf",
            "/home/ana/docs/letters/lease.txt",
            "/home/ana/docs/Ärzte/brief.txt",
            "/home/ana/docs/letters/notes.md",
        ]);
        assert_eq!(
            found_names(&store, "march invoice"),
            vec!["/home/ana/docs/Taxes 2023/invoice-march.pdf"]
        );
        // A folder's name finds what is in it.
        assert_eq!(found_names(&store, "taxes").len(), 2);
        // Accents and case do not matter.
        assert_eq!(
            found_names(&store, "arzte"),
            vec!["/home/ana/docs/Ärzte/brief.txt"]
        );
        // A question with other words in it still finds the file by the
        // two words that are in its name.
        assert_eq!(
            found_names(&store, "the invoice we paid in march"),
            vec!["/home/ana/docs/Taxes 2023/invoice-march.pdf"]
        );
        // Words in every path say nothing.
        assert!(found_names(&store, "docs ana").is_empty());
    }

    #[test]
    fn a_file_found_by_its_name_alone_says_so_and_has_its_date() {
        let store = named(&[
            "/docs/plumber-invoice.pdf",
            "/docs/lease.txt",
            "/docs/notes.txt",
        ]);
        let results = store
            .search_with_names("plumber", None, 10, &Filter::default())
            .unwrap();
        assert_eq!(results.len(), 1);
        let (hit, found) = &results[0];
        assert_eq!(*found, Found::Name);
        assert_eq!(hit.path, "/docs/plumber-invoice.pdf");
        assert_eq!(hit.modified_secs, 1_700_000_000);
        assert!(hit.snippet.starts_with("passage number 0"));
        // The evaluation's search reads passages only.
        assert!(store
            .search_combined("plumber", None, 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn removed_files_leave_the_name_index() {
        let mut store = named(&["/docs/plumber-invoice.pdf", "/docs/a.txt", "/docs/b.txt"]);
        store
            .purge_missing(
                "/docs/",
                &["/docs/a.txt".to_string(), "/docs/b.txt".to_string()],
            )
            .unwrap();
        assert!(found_names(&store, "plumber").is_empty());
    }

    #[test]
    fn a_version_4_index_gets_its_names_indexed() {
        let file = std::env::temp_dir().join("catchword-store-test-names-upgrade.db");
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
        {
            let mut store = Store::open(&file).unwrap();
            for (path, hash) in [
                ("/d/plumber.txt", "h1"),
                ("/d/a.txt", "h2"),
                ("/d/b.txt", "h3"),
            ] {
                store
                    .put_file(path, 1, 1, hash, &chunk("some text", 50, 5, &WordTokenizer))
                    .unwrap();
            }
            // Make it look like version 4, which had no name index.
            store
                .conn
                .execute_batch(
                    "DROP TRIGGER files_name_added;
                     DROP TRIGGER files_name_removed;
                     DROP TRIGGER files_name_changed;
                     DROP TABLE names_fts;
                     UPDATE meta SET value = '4' WHERE key = 'schema_version';",
                )
                .unwrap();
        }
        let store = Store::open(&file).unwrap();
        assert!(!store.was_reset());
        assert_eq!(found_names(&store, "plumber"), vec!["/d/plumber.txt"]);
    }

    #[test]
    fn the_last_scan_is_remembered() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.last_scan().unwrap(), None);
        store.set_last_scan(1_790_000_000).unwrap();
        assert_eq!(store.last_scan().unwrap(), Some(1_790_000_000));
    }

    #[test]
    fn a_sound_index_passes_both_checks() {
        let store = named(&["/docs/a.txt", "/docs/b.txt"]);
        assert!(store.quick_check().unwrap());
        assert!(store.integrity_check().unwrap());
    }

    #[test]
    fn a_keyword_index_out_of_step_fails_the_full_check() {
        let store = named(&["/docs/a.txt", "/docs/b.txt"]);
        // Remove a passage behind the keyword index's back.
        store
            .conn
            .execute(
                "DELETE FROM passages WHERE id = (SELECT MIN(id) FROM passages)",
                (),
            )
            .unwrap();
        assert!(store.quick_check().unwrap());
        assert!(!store.integrity_check().unwrap());
    }

    #[test]
    fn a_newer_index_is_told_apart_from_a_damaged_one() {
        let file = std::env::temp_dir().join("catchword-store-test-newer-kind.db");
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
        {
            let store = Store::open(&file).unwrap();
            meta_set(
                &store.conn,
                "schema_version",
                &(SCHEMA_VERSION + 1).to_string(),
            )
            .unwrap();
        }
        let error = Store::open(&file).err().unwrap();
        assert!(is_newer_layout(&error), "{error}");

        std::fs::write(&file, b"not a database at all, just some bytes").unwrap();
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
        let error = Store::open(&file).err().unwrap();
        assert!(!is_newer_layout(&error), "{error}");
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

    #[test]
    fn a_date_filter_keeps_recent_files_and_shows_the_recent_copy() {
        let mut store = Store::open_in_memory().unwrap();
        for (path, modified, hash, text) in [
            (
                "/old/lease.txt",
                1_000,
                "h-lease",
                "the notice period is three months",
            ),
            (
                "/recent/lease-copy.txt",
                9_000,
                "h-lease",
                "the notice period is three months",
            ),
            (
                "/old/notice.txt",
                1_000,
                "h-other",
                "a notice about the old office",
            ),
        ] {
            let passages = chunk(text, 50, 5, &WordTokenizer);
            store.put_file(path, 1, modified, hash, &passages).unwrap();
        }
        let recent = Filter {
            modified_since: Some(5_000),
            ..Filter::default()
        };
        assert!(!recent.is_empty());
        let found = store
            .search_with_names("notice", None, 10, &recent)
            .unwrap();
        let paths: Vec<&str> = found.iter().map(|(hit, _)| hit.path.as_str()).collect();
        assert_eq!(paths, vec!["/recent/lease-copy.txt"]);
        assert_eq!(found[0].0.modified_secs, 9_000);
        let all = store
            .search_with_names("notice", None, 10, &Filter::default())
            .unwrap();
        assert_eq!(all.len(), 2);
    }

    /// Where `needle` appears in the bytes of the index file and its log.
    fn traces(file: &Path, needle: &[u8]) -> Vec<String> {
        let mut found = Vec::new();
        for suffix in ["", "-wal"] {
            let Ok(bytes) = std::fs::read(format!("{}{suffix}", file.display())) else {
                continue;
            };
            if bytes.windows(needle.len()).any(|w| w == needle) {
                found.push(format!("index{suffix}"));
            }
        }
        found
    }

    /// How a file leaves the index, in [`a_purged_file_leaves_no_trace_in_the_index_file`].
    #[derive(Debug, Clone, Copy)]
    enum Leaving {
        /// Deleted with others: one of two files, so a bulk purge.
        Bulk,
        /// Deleted alone from a larger index: one of a hundred files.
        Alone,
        /// Its content changed: the old text goes when the new one is written.
        Changed,
        /// Passages are cut another way, so everything goes.
        NewPipeline,
    }

    /// PRIV-5: once a file is purged, nothing of it can be read back from the
    /// index: not its text, its words, its name or its vectors.
    #[test]
    fn a_purged_file_leaves_no_trace_in_the_index_file() {
        for leaving in [
            Leaving::Bulk,
            Leaving::Alone,
            Leaving::Changed,
            Leaving::NewPipeline,
        ] {
            leaves_no_trace(leaving);
        }
    }

    fn leaves_no_trace(leaving: Leaving) {
        let file = std::env::temp_dir().join(format!("catchword-store-test-purged-{leaving:?}.db"));
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
        let vector: Vec<f32> = vec![0.123_456_7, -0.765_432_1, 0.333_111_9, 0.271_828_2];
        let secret = "Xylophonequartz met Brontosaurusfig at the quayside";
        let others = match leaving {
            Leaving::Alone => 99,
            _ => 1,
        };
        let kept: Vec<String> = (0..others).map(|n| format!("/docs/kept-{n}.txt")).collect();
        {
            let mut store = Store::open(&file).unwrap();
            store.use_model("test", 4).unwrap();
            store.use_pipeline("first way").unwrap();
            for (n, path) in kept.iter().enumerate() {
                let text = format!("an ordinary note about lunch number {n}");
                let passages = chunk(&text, 50, 5, &WordTokenizer);
                store
                    .put_file(path, 1, 1, &format!("h-kept-{n}"), &passages)
                    .unwrap();
            }
            let passages = chunk(secret, 50, 5, &WordTokenizer);
            store
                .put_file("/docs/quixoticname.txt", 1, 1, "h-secret", &passages)
                .unwrap();
            let id = store.search_keyword("xylophonequartz", 1).unwrap()[0].passage_id;
            store.put_vectors(&[(id, vector.clone())]).unwrap();
        }
        let blob = as_blob(&vector);
        let needles: [(&str, &[u8]); 5] = [
            ("text", secret.as_bytes()),
            ("word", b"xylophonequartz"),
            ("second word", b"brontosaurusfig"),
            ("name", b"quixoticname"),
            ("vector", &blob),
        ];
        // Before the purge every trace is there, so the search below can see them.
        for (what, needle) in needles {
            assert!(
                !traces(&file, needle).is_empty(),
                "{leaving:?}: {what} not found before"
            );
        }
        // A changed file keeps its name: only its old content must go.
        let expected: Vec<String> = match leaving {
            Leaving::Changed => vec!["name in index".to_string()],
            _ => Vec::new(),
        };
        {
            let mut store = Store::open(&file).unwrap();
            match leaving {
                Leaving::Bulk | Leaving::Alone => {
                    store.purge_missing("/docs/", &kept).unwrap();
                    // The other files are untouched.
                    assert_eq!(store.counts().unwrap().files, others as i64);
                }
                Leaving::Changed => {
                    // The file is rewritten; then the scan ends, as every
                    // scan does, by purging what it did not see.
                    let passages = chunk("a plain new text", 50, 5, &WordTokenizer);
                    store
                        .put_file("/docs/quixoticname.txt", 2, 2, "h-new", &passages)
                        .unwrap();
                    let mut seen = kept.clone();
                    seen.push("/docs/quixoticname.txt".to_string());
                    store.purge_missing("/docs/", &seen).unwrap();
                    assert_eq!(store.search_keyword("plain", 5).unwrap().len(), 1);
                }
                Leaving::NewPipeline => {
                    assert!(store.use_pipeline("another way").unwrap());
                    assert_eq!(store.counts().unwrap(), Counts::default());
                }
            }
            assert!(store
                .search_keyword("xylophonequartz", 5)
                .unwrap()
                .is_empty());
            assert_eq!(
                left_behind(&file, &needles),
                expected,
                "{leaving:?}, still open"
            );
        }
        assert_eq!(
            left_behind(&file, &needles),
            expected,
            "{leaving:?}, closed"
        );
    }

    /// An index made before deleted words were dropped at once still holds
    /// them; the next start drops them.
    #[test]
    fn words_an_older_index_kept_are_dropped_when_it_is_opened() {
        let file = std::env::temp_dir().join("catchword-store-test-older-purge.db");
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
        let needles: [(&str, &[u8]); 2] = [("word", b"xylophonequartz"), ("name", b"quixoticname")];
        {
            let mut store = Store::open(&file).unwrap();
            // As an index made before: deleted entries left in place.
            for table in ["passages_fts", "names_fts"] {
                store
                    .conn
                    .execute(
                        &format!("INSERT INTO {table}({table}, rank) VALUES ('secure-delete', 0)"),
                        (),
                    )
                    .unwrap();
            }
            for (path, hash, text) in [
                ("/docs/kept.txt", "h-kept", "an ordinary note about lunch"),
                (
                    "/docs/quixoticname.txt",
                    "h-gone",
                    "Xylophonequartz at the quayside",
                ),
            ] {
                let passages = chunk(text, 50, 5, &WordTokenizer);
                store.put_file(path, 1, 1, hash, &passages).unwrap();
            }
            // Deleted as before: entries left in the keyword indexes.
            let tx = store.conn.transaction().unwrap();
            tx.execute(
                "DELETE FROM files WHERE path = '/docs/quixoticname.txt'",
                (),
            )
            .unwrap();
            drop_orphans(&tx).unwrap();
            tx.commit().unwrap();
            checkpoint(&store.conn).unwrap();
        }
        assert_eq!(
            left_behind(&file, &needles),
            ["word in index", "name in index"],
            "the older index should still hold them"
        );
        let store = Store::open(&file).unwrap();
        assert_eq!(left_behind(&file, &needles), Vec::<String>::new());
        assert_eq!(store.search_keyword("lunch", 5).unwrap().len(), 1);
        // The remaining file's name is still indexed. (Name search itself
        // would ignore a word that is in every file.)
        let named: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM names_fts WHERE names_fts MATCH 'kept'",
                (),
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(named, 1);
    }

    /// Which of `needles` can still be read from the index file or its log.
    fn left_behind(file: &Path, needles: &[(&str, &[u8])]) -> Vec<String> {
        needles
            .iter()
            .flat_map(|(what, needle)| {
                traces(file, needle)
                    .into_iter()
                    .map(move |place| format!("{what} in {place}"))
            })
            .collect()
    }

    /// How long deleting takes, for ADR-22. Run with
    /// `cargo test --release -p catchword-store measure_deleting -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_deleting() {
        const FILES: usize = 10_000; // ten passages each
        let file = std::env::temp_dir().join("catchword-store-measure-deleting.db");
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
        let mut store = Store::open(&file).unwrap();
        store.use_pipeline("first way").unwrap();
        // Words drawn as in real text: a few common, most rare (Zipf's law).
        let mut state: u64 = 7;
        let mut word = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let rank = ((state >> 33) % 50_000) as f64;
            format!("w{}", rank.powf(1.3) as u32)
        };
        let path = |n: usize| format!("/docs/{}/{n}.txt", n % 2);
        for n in 0..FILES {
            let text: Vec<String> = (0..2_500).map(|_| word()).collect();
            let passages = chunk(&text.join(" "), 250, 0, &WordTokenizer);
            store
                .put_file(&path(n), 1, 1, &format!("h{n}"), &passages)
                .unwrap();
        }
        let passages = store.counts().unwrap().passages;
        let all: Vec<String> = (0..FILES).map(path).collect();
        let time = |what: &str, started: std::time::Instant| {
            println!("{what}: {:?}", started.elapsed());
        };

        let started = std::time::Instant::now();
        store.purge_missing("/docs/", &all[1..]).unwrap();
        time("one file deleted, in place", started);

        let started = std::time::Instant::now();
        let text: Vec<String> = (0..2_500).map(|_| word()).collect();
        let changed = chunk(&text.join(" "), 250, 0, &WordTokenizer);
        store
            .put_file(&all[1], 2, 2, "h-changed", &changed)
            .unwrap();
        time("one file changed, in place", started);

        let started = std::time::Instant::now();
        let odd: Vec<String> = all[1..]
            .iter()
            .filter(|p| p.contains("/1/"))
            .cloned()
            .collect();
        store.purge_missing("/docs/", &odd).unwrap();
        time("half the files deleted, then compacted", started);

        let started = std::time::Instant::now();
        store.use_pipeline("another way").unwrap();
        time("everything deleted, then compacted", started);
        println!("from {passages} passages in {FILES} files");
    }
}
