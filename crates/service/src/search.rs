//! Search by words and by meaning, combined by rank (ADR-5, ADR-20).

use anyhow::{Context, Result};
use catchword_engine::fuse::{Found, CANDIDATES};
use catchword_store::{Filter, Hit, Store};

use crate::{lock, Model};

/// Something the user should know about a search's results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// No model: results are by words only.
    MeaningOff(String),
    /// The index has no vectors yet: results are by words only.
    NotEmbeddedYet,
    /// The index was embedded with another model: results are by words only.
    OtherModel(String),
    /// Some passages are not searchable by meaning yet.
    Partial { searchable: i64, passages: i64 },
}

impl Note {
    pub fn describe(&self) -> String {
        match self {
            Note::MeaningOff(why) => format!("meaning search is off: {why}."),
            Note::NotEmbeddedYet => {
                "this index is not searchable by meaning yet. Index it again.".to_string()
            }
            Note::OtherModel(model) => format!(
                "this index was embedded with another model ({model}). \
                 Index it again to update it. Showing matches by words only."
            ),
            Note::Partial {
                searchable,
                passages,
            } => format!("{searchable} of {passages} passages are searchable by meaning so far."),
        }
    }
}

/// The results of one search, best first, and what to know about them.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub results: Vec<(Hit, Found)>,
    pub notes: Vec<Note>,
}

pub fn search(store: &Store, model: &Model, query: &str) -> Result<Answer> {
    search_within(store, model, query, &Filter::default())
}

/// As `search`, with results only from files `filter` allows (SEA-6).
pub fn search_within(store: &Store, model: &Model, query: &str, filter: &Filter) -> Result<Answer> {
    let mut notes = Vec::new();
    let vector = match model {
        Model::Unavailable(why) => {
            notes.push(Note::MeaningOff(why.clone()));
            None
        }
        Model::Ready(model) => match store.embedding_model()? {
            None => {
                notes.push(Note::NotEmbeddedYet);
                None
            }
            Some(indexed_with) if indexed_with != lock(model).manifest().id() => {
                notes.push(Note::OtherModel(indexed_with));
                None
            }
            Some(_) => {
                let counts = store.counts()?;
                if counts.vectors < counts.passages {
                    notes.push(Note::Partial {
                        searchable: counts.vectors,
                        passages: counts.passages,
                    });
                }
                Some(
                    lock(model)
                        // Quotes ask for exact words; the meaning is in the words.
                        .embed_query(&query.replace(['"', '\u{201c}', '\u{201d}'], ""))
                        .context("cannot embed the query")?,
                )
            }
        },
    };
    let results = store.search_with_names(query, vector.as_deref(), CANDIDATES, filter)?;
    Ok(Answer { results, notes })
}

/// One file's matching passages, best first.
#[derive(Debug, Clone, PartialEq)]
pub struct FileResults {
    pub path: String,
    /// How many files share this exact content (1 means no copies).
    pub copies: i64,
    /// When the file was last changed, in seconds since 1970.
    pub modified_secs: i64,
    pub passages: Vec<(Hit, Found)>,
}

/// Group results by file, in the order each file first appears, so one long
/// document cannot fill the list (SEA-3).
pub fn group_by_file(results: Vec<(Hit, Found)>) -> Vec<FileResults> {
    let mut files: Vec<FileResults> = Vec::new();
    for (hit, found) in results {
        match files.iter_mut().find(|file| file.path == hit.path) {
            Some(file) => file.passages.push((hit, found)),
            None => files.push(FileResults {
                path: hit.path.clone(),
                copies: hit.copies,
                modified_secs: hit.modified_secs,
                passages: vec![(hit, found)],
            }),
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(id: i64, path: &str) -> (Hit, Found) {
        let hit = Hit {
            passage_id: id,
            path: path.to_string(),
            copies: 1,
            page: None,
            start_line: 1,
            end_line: 1,
            snippet: String::new(),
            score: 0.0,
            modified_secs: 0,
        };
        (hit, Found::Keyword)
    }

    #[test]
    fn results_are_grouped_by_file_in_first_seen_order() {
        let grouped = group_by_file(vec![hit(1, "/b.txt"), hit(2, "/a.txt"), hit(3, "/b.txt")]);
        let shape: Vec<(&str, Vec<i64>)> = grouped
            .iter()
            .map(|file| {
                let ids = file.passages.iter().map(|(h, _)| h.passage_id).collect();
                (file.path.as_str(), ids)
            })
            .collect();
        assert_eq!(shape, vec![("/b.txt", vec![1, 3]), ("/a.txt", vec![2])]);
    }

    #[test]
    fn without_a_model_results_are_by_words_and_say_so() {
        let store = Store::open_in_memory().unwrap();
        let answer = search(&store, &Model::Unavailable("not installed".into()), "x").unwrap();
        assert_eq!(answer.notes, vec![Note::MeaningOff("not installed".into())]);
        assert_eq!(
            answer.notes[0].describe(),
            "meaning search is off: not installed."
        );
    }
}
