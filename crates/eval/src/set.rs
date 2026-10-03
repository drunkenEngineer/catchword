//! Evaluation set version 1 (ADR-19): documents and judged queries.
//!
//! The XQuAD part is built from the pinned download the same way every time;
//! the written part is read from `eval/domain/`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// A question or description in the document's own language.
    Descriptive,
    /// A name or number to look up.
    Exact,
    /// A question in another language than the document's.
    CrossLanguage,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Descriptive => "descriptive",
            Kind::Exact => "exact",
            Kind::CrossLanguage => "cross-language",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [Kind::Descriptive, Kind::Exact, Kind::CrossLanguage]
            .into_iter()
            .find(|kind| kind.name() == text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Xquad,
    Domain,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Xquad => "XQuAD",
            Source::Domain => "written",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    /// The file name the document is indexed under.
    pub name: String,
    pub language: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub source: Source,
    pub kind: Kind,
    /// The language the query is written in.
    pub language: String,
    pub text: String,
    /// The document that answers it, and the line the answer is on.
    pub doc: String,
    pub doc_language: String,
    pub line: u32,
    /// Text a relevant passage must contain, in the document's language.
    pub answer: String,
}

#[derive(Debug, Default)]
pub struct EvalSet {
    pub docs: Vec<Doc>,
    pub queries: Vec<Query>,
}

/// The languages XQuAD articles are written in, in turn: article 0 in
/// English, 1 in German, 2 in Arabic, 3 in English again, and so on.
pub const XQUAD_LANGUAGES: [&str; 3] = ["en", "de", "ar"];

/// The whole set: XQuAD from `xquad`, the written part from `domain`.
pub fn load(xquad: &Path, domain: &Path) -> Result<EvalSet> {
    let mut set = from_xquad(xquad)?;
    add_exact_queries(&mut set);
    let written = from_domain(domain)?;
    set.docs.extend(written.docs);
    set.queries.extend(written.queries);
    Ok(set)
}

fn from_xquad(folder: &Path) -> Result<EvalSet> {
    let mut files = Vec::new();
    for language in XQUAD_LANGUAGES {
        let path = folder.join(format!("xquad.{language}.json"));
        let text = fs::read_to_string(&path).with_context(|| {
            format!(
                "cannot read {}; run sh scripts/fetch-eval.sh",
                path.display()
            )
        })?;
        let json: Value = serde_json::from_str(&text)?;
        files.push(json);
    }
    let articles = |file: usize| -> Result<&Vec<Value>> {
        files[file]["data"]
            .as_array()
            .context("XQuAD file without articles")
    };

    let mut set = EvalSet::default();
    for index in 0..articles(0)?.len() {
        let own = index % XQUAD_LANGUAGES.len();
        let language = XQUAD_LANGUAGES[own];
        // English documents are asked about in Arabic; the others in English.
        let other = if own == 0 { 2 } else { 0 };
        let name = format!("xquad-{language}-{index:02}.txt");
        let paragraphs = list(&articles(own)?[index], "paragraphs")?;
        let translated = list(&articles(other)?[index], "paragraphs")?;

        let mut lines = Vec::new();
        for (number, paragraph) in paragraphs.iter().enumerate() {
            lines.push(one_line(text(paragraph, "context")?));
            let line = number as u32 + 1;
            let questions = list(paragraph, "qas")?;
            let other_questions = list(&translated[number], "qas")?;
            for (question, other_question) in questions.iter().zip(other_questions) {
                let answer = one_line(text(&list(question, "answers")?[0], "text")?);
                for (kind, query_language, query) in [
                    (Kind::Descriptive, language, question),
                    (Kind::CrossLanguage, XQUAD_LANGUAGES[other], other_question),
                ] {
                    set.queries.push(Query {
                        source: Source::Xquad,
                        kind,
                        language: query_language.to_string(),
                        text: text(query, "question")?.to_string(),
                        doc: name.clone(),
                        doc_language: language.to_string(),
                        line,
                        answer: answer.clone(),
                    });
                }
            }
        }
        set.docs.push(Doc {
            name,
            language: language.to_string(),
            text: lines.join("\n"),
        });
    }
    Ok(set)
}

fn list<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    value[key]
        .as_array()
        .with_context(|| format!("XQuAD entry without {key}"))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .with_context(|| format!("XQuAD entry without {key}"))
}

/// Text on one line with single spaces, as passages hold it.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Names and numbers found in exactly one paragraph of the XQuAD documents
/// become exact queries: one for every second paragraph that has one.
fn add_exact_queries(set: &mut EvalSet) {
    let mut paragraphs_with: HashMap<String, usize> = HashMap::new();
    for doc in &set.docs {
        for line in doc.text.lines() {
            let words: HashSet<String> = names_and_numbers(line)
                .map(|word| word.to_lowercase())
                .collect();
            for word in words {
                *paragraphs_with.entry(word).or_default() += 1;
            }
        }
    }
    let mut exact = Vec::new();
    for doc in &set.docs {
        for (index, line) in doc.text.lines().enumerate() {
            if index % 2 != 0 {
                continue;
            }
            let unique = names_and_numbers(line)
                .find(|word| paragraphs_with.get(&word.to_lowercase()) == Some(&1));
            if let Some(word) = unique {
                exact.push(Query {
                    source: Source::Xquad,
                    kind: Kind::Exact,
                    language: doc.language.clone(),
                    text: word.to_string(),
                    doc: doc.name.clone(),
                    doc_language: doc.language.clone(),
                    line: index as u32 + 1,
                    answer: word.to_string(),
                });
            }
        }
    }
    set.queries.extend(exact);
}

/// Words with a digit (at least four characters, such as a year) or a
/// capital letter (at least five), without surrounding punctuation.
fn names_and_numbers(line: &str) -> impl Iterator<Item = &str> {
    line.split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|word| {
            let length = word.chars().count();
            let digit = word.chars().any(|c| c.is_ascii_digit());
            let capital = word.chars().next().is_some_and(char::is_uppercase);
            (digit && length >= 4) || (capital && length >= 5)
        })
}

/// The written part: documents in `docs/`, named `<language>-<topic>.txt`,
/// and `queries.tsv` with the columns kind, language, document, answer,
/// query. The answer must occur on exactly one line of its document.
fn from_domain(folder: &Path) -> Result<EvalSet> {
    let mut set = EvalSet::default();
    let mut names: Vec<_> = fs::read_dir(folder.join("docs"))
        .with_context(|| format!("cannot read {}", folder.join("docs").display()))?
        .filter_map(|entry| {
            entry
                .ok()
                .map(|e| e.file_name().to_string_lossy().to_string())
        })
        .filter(|name| name.ends_with(".txt"))
        .collect();
    names.sort();
    for name in names {
        let text = fs::read_to_string(folder.join("docs").join(&name))?;
        let language = name.split('-').next().unwrap_or_default().to_string();
        set.docs.push(Doc {
            name,
            language,
            text,
        });
    }

    let queries = fs::read_to_string(folder.join("queries.tsv"))?;
    for (number, row) in queries.lines().enumerate() {
        if row.trim().is_empty() || row.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = row.split('\t').collect();
        let [kind, language, doc, answer, query] = columns[..] else {
            bail!("queries.tsv line {}: expected 5 columns", number + 1);
        };
        let Some(kind) = Kind::parse(kind) else {
            bail!("queries.tsv line {}: unknown kind {kind}", number + 1);
        };
        let Some(document) = set.docs.iter().find(|d| d.name == doc) else {
            bail!("queries.tsv line {}: no document {doc}", number + 1);
        };
        let lines: Vec<usize> = document
            .text
            .lines()
            .enumerate()
            .filter(|(_, line)| one_line(line).contains(&one_line(answer)))
            .map(|(index, _)| index + 1)
            .collect();
        let [line] = lines[..] else {
            bail!(
                "queries.tsv line {}: \"{answer}\" is on {} lines of {doc}, not one",
                number + 1,
                lines.len()
            );
        };
        set.queries.push(Query {
            source: Source::Domain,
            kind,
            language: language.to_string(),
            text: query.to_string(),
            doc: doc.to_string(),
            doc_language: document.language.clone(),
            line: line as u32,
            answer: one_line(answer),
        });
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_numbers_are_found_without_punctuation() {
        let found: Vec<&str> =
            names_and_numbers("In 1879, Nikola Tesla moved to Graz (Austria); the 12 men.")
                .collect();
        assert_eq!(found, vec!["1879", "Nikola", "Tesla", "Austria"]);
    }

    #[test]
    fn exact_queries_use_words_found_in_one_paragraph_only() {
        let mut set = EvalSet {
            docs: vec![
                Doc {
                    name: "a.txt".into(),
                    language: "en".into(),
                    text: "Vienna is where Tesla studied.\nsecond line".into(),
                },
                Doc {
                    name: "b.txt".into(),
                    language: "en".into(),
                    text: "Tesla later moved to Prague in 1880.".into(),
                },
            ],
            queries: Vec::new(),
        };
        add_exact_queries(&mut set);
        let found: Vec<(&str, &str, u32)> = set
            .queries
            .iter()
            .map(|q| (q.text.as_str(), q.doc.as_str(), q.line))
            .collect();
        // "Tesla" is in two paragraphs, so it is not used.
        assert_eq!(found, vec![("Vienna", "a.txt", 1), ("Prague", "b.txt", 1)]);
    }

    #[test]
    fn the_written_part_reads_documents_and_queries() {
        let folder = std::env::temp_dir().join("catchword-eval-test-domain");
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(folder.join("docs")).unwrap();
        fs::write(
            folder.join("docs/fr-bail.txt"),
            "Contrat de bail\nLe préavis est de trois mois.",
        )
        .unwrap();
        fs::write(
            folder.join("queries.tsv"),
            "# kind\tlanguage\tdocument\tanswer\tquery\n\
             cross-language\ten\tfr-bail.txt\ttrois mois\tlease notice period\n",
        )
        .unwrap();
        let set = from_domain(&folder).unwrap();
        assert_eq!(set.docs[0].language, "fr");
        let query = &set.queries[0];
        assert_eq!(query.kind, Kind::CrossLanguage);
        assert_eq!((query.line, query.doc_language.as_str()), (2, "fr"));

        // An answer that is not in its document is refused.
        fs::write(
            folder.join("queries.tsv"),
            "exact\tfr\tfr-bail.txt\tINV-1\tINV-1\n",
        )
        .unwrap();
        assert!(from_domain(&folder).is_err());
        fs::remove_dir_all(&folder).unwrap();
    }
}
