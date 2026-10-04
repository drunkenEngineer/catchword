//! Index the evaluation set with one model and passage size, ask every
//! query, and judge the results. Uses the same code as the app: the
//! engine's passage cutting, the store's searches and the embedding runtime.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use catchword_embed::{runtime_library_name, Embedder, ModelManifest, Paths};
use catchword_engine::{chunk, content_hash, Passage};
use catchword_store::{Hit, Store};

use crate::metrics::{Judged, K};
use crate::set::{one_line, EvalSet, Query};

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub model: ModelManifest,
    pub tokens: usize,
    pub overlap: usize,
    /// Results each kind of search contributes to the combined list.
    pub candidates: usize,
}

impl Config {
    pub fn label(&self) -> String {
        format!(
            "{}, {} tokens (overlap {}), {} candidates",
            self.model.name, self.tokens, self.overlap, self.candidates
        )
    }
}

/// The three ways of searching that are scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Search {
    Words,
    Meaning,
    Combined,
}

impl Search {
    pub const ALL: [Search; 3] = [Search::Combined, Search::Words, Search::Meaning];

    pub fn name(self) -> &'static str {
        match self {
            Search::Words => "words",
            Search::Meaning => "meaning",
            Search::Combined => "combined",
        }
    }
}

/// What one configuration did.
pub struct Outcome {
    pub config: Config,
    pub passages: usize,
    pub load_seconds: f64,
    pub embed_seconds: f64,
    /// Total time spent embedding queries.
    pub query_seconds: f64,
    /// For each query of the set, in order, the judged results of each search.
    pub judged: Vec<HashMap<Search, Judged>>,
}

impl Outcome {
    pub fn passages_per_second(&self) -> f64 {
        self.passages as f64 / self.embed_seconds
    }
}

/// The runtime and a model, from the source tree's vendor/ folder.
pub fn vendor_paths(vendor: &Path, model: &ModelManifest) -> Paths {
    Paths {
        runtime: vendor.join("onnxruntime").join(runtime_library_name()),
        model: vendor.join("models").join(model.name),
    }
}

pub fn evaluate(set: &EvalSet, config: &Config, vendor: &Path) -> Result<Outcome> {
    let started = Instant::now();
    let mut model = Embedder::load(
        &vendor_paths(vendor, &config.model),
        &config.model,
        catchword_embed::Threads::RUNTIME_DEFAULT,
    )
    .with_context(|| format!("cannot load {}", config.model.name))?;
    let load_seconds = started.elapsed().as_secs_f64();

    let mut store = Store::open_in_memory()?;
    store.use_model(&config.model.id(), config.model.dimensions)?;
    let mut cut: HashMap<&str, Vec<Passage>> = HashMap::new();
    for doc in &set.docs {
        let passages = chunk(&doc.text, config.tokens, config.overlap, &model);
        store.put_file(
            &doc.name,
            doc.text.len() as u64,
            0,
            &content_hash(doc.text.as_bytes()),
            &passages,
        )?;
        cut.insert(doc.name.as_str(), passages);
    }

    let started = Instant::now();
    loop {
        let batch = store.passages_without_vectors(64)?;
        if batch.is_empty() {
            break;
        }
        let texts: Vec<&str> = batch.iter().map(|(_, text)| text.as_str()).collect();
        let vectors = model.embed_passages(&texts)?;
        let pairs: Vec<(i64, Vec<f32>)> = batch.iter().map(|(id, _)| *id).zip(vectors).collect();
        store.put_vectors(&pairs)?;
    }
    let embed_seconds = started.elapsed().as_secs_f64();
    let passages = store.counts()?.passages as usize;

    let mut query_seconds = 0.0;
    let mut judged = Vec::with_capacity(set.queries.len());
    for query in &set.queries {
        let total = cut[query.doc.as_str()]
            .iter()
            .filter(|p| {
                is_relevant(
                    query,
                    &query.doc,
                    p.start_line.into(),
                    p.end_line.into(),
                    &p.text,
                )
            })
            .count();
        let started = Instant::now();
        let vector = model.embed_query(&query.text)?;
        query_seconds += started.elapsed().as_secs_f64();

        let words = store.search_keyword(&query.text, K)?;
        let meaning = store.search_vector(&vector, K)?;
        let combined: Vec<Hit> = store
            .search_combined(&query.text, Some(&vector), config.candidates)?
            .into_iter()
            .take(K)
            .map(|(hit, _)| hit)
            .collect();
        let mut one = HashMap::new();
        for (search, hits) in [
            (Search::Words, words),
            (Search::Meaning, meaning),
            (Search::Combined, combined),
        ] {
            let relevant = hits
                .iter()
                .map(|hit| judge(&store, query, hit))
                .collect::<Result<Vec<bool>>>()?;
            one.insert(search, Judged { relevant, total });
        }
        judged.push(one);
    }

    Ok(Outcome {
        config: *config,
        passages,
        load_seconds,
        embed_seconds,
        query_seconds,
        judged,
    })
}

fn judge(store: &Store, query: &Query, hit: &Hit) -> Result<bool> {
    let text = store.passage_text(hit.passage_id)?.unwrap_or_default();
    Ok(is_relevant(
        query,
        &hit.path,
        hit.start_line,
        hit.end_line,
        &text,
    ))
}

/// A passage is relevant when it is from the right document, covers the
/// answer's line and contains the whole answer (ADR-19).
fn is_relevant(query: &Query, doc: &str, start_line: i64, end_line: i64, text: &str) -> bool {
    let line = i64::from(query.line);
    doc == query.doc
        && start_line <= line
        && line <= end_line
        && one_line(text).contains(&query.answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::set::{Kind, Source};

    fn query() -> Query {
        Query {
            source: Source::Domain,
            kind: Kind::Exact,
            language: "en".into(),
            text: "INV-2026-0042".into(),
            doc: "en-invoice.txt".into(),
            doc_language: "en".into(),
            line: 3,
            answer: "INV-2026-0042".into(),
        }
    }

    #[test]
    fn relevance_needs_the_document_the_line_and_the_whole_answer() {
        let q = query();
        assert!(is_relevant(
            &q,
            "en-invoice.txt",
            2,
            4,
            "Invoice INV-2026-0042 due"
        ));
        assert!(!is_relevant(
            &q,
            "other.txt",
            2,
            4,
            "Invoice INV-2026-0042 due"
        ));
        assert!(!is_relevant(
            &q,
            "en-invoice.txt",
            4,
            6,
            "Invoice INV-2026-0042 due"
        ));
        // Cut in half at a passage boundary: in neither passage.
        assert!(!is_relevant(&q, "en-invoice.txt", 2, 3, "Invoice INV-2026"));
    }
}
