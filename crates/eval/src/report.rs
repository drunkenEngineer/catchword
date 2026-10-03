//! Scores as Markdown tables, and the threshold check.

use std::fmt::Write;

use anyhow::{bail, Context, Result};

use crate::metrics::Scores;
use crate::run::{Outcome, Search};
use crate::set::{EvalSet, Kind, Query, Source};

/// Scores of one search over the queries that `keep` accepts.
pub fn scores(
    set: &EvalSet,
    outcome: &Outcome,
    search: Search,
    keep: impl Fn(&Query) -> bool,
) -> Scores {
    Scores::of(
        set.queries
            .iter()
            .zip(&outcome.judged)
            .filter(|(query, _)| keep(query))
            .map(|(_, judged)| &judged[&search]),
    )
}

/// Decides whether a query belongs to a group.
pub type Filter = Box<dyn Fn(&Query) -> bool>;

/// A named group of queries, as used in tables and in thresholds.
pub fn group(name: &str) -> Result<Filter> {
    let owned = name.to_string();
    Ok(match name.split_once(':') {
        None if name == "all" => Box::new(|_| true),
        Some(("kind", kind)) => {
            let kind = kind.to_string();
            Box::new(move |q: &Query| q.kind.name() == kind)
        }
        Some(("language", language)) => {
            let language = language.to_string();
            Box::new(move |q: &Query| q.doc_language == language)
        }
        Some(("script", "latin")) => Box::new(|q: &Query| q.doc_language != "ar"),
        Some(("source", source)) => {
            let source = source.to_string();
            Box::new(move |q: &Query| q.source.name() == source)
        }
        _ => bail!("unknown group {owned}"),
    })
}

fn groups(set: &EvalSet) -> Vec<String> {
    let mut names = vec!["all".to_string()];
    for kind in [Kind::Descriptive, Kind::Exact, Kind::CrossLanguage] {
        names.push(format!("kind:{}", kind.name()));
    }
    let mut languages: Vec<&str> = set
        .queries
        .iter()
        .map(|q| q.doc_language.as_str())
        .collect();
    languages.sort();
    languages.dedup();
    for language in languages {
        names.push(format!("language:{language}"));
    }
    names.push("script:latin".to_string());
    for source in [Source::Xquad, Source::Domain] {
        names.push(format!("source:{}", source.name()));
    }
    names
}

/// The full report of one configuration.
pub fn detail(set: &EvalSet, outcome: &Outcome) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "### {}\n", outcome.config.label());
    let _ = writeln!(
        out,
        "{} passages. Model loaded in {:.1} s. Embedding: {:.1} passages a second. \
         Query embedding: {:.0} ms on average.\n",
        outcome.passages,
        outcome.load_seconds,
        outcome.passages_per_second(),
        1000.0 * outcome.query_seconds / set.queries.len().max(1) as f64
    );
    let _ = writeln!(
        out,
        "| Queries | Count | Combined recall@10 | Combined MRR@10 | Combined nDCG@10 \
         | Words recall@10 | Meaning recall@10 |"
    );
    let _ = writeln!(out, "| --- | ---: | ---: | ---: | ---: | ---: | ---: |");
    for name in groups(set) {
        let Ok(keep) = group(&name) else { continue };
        let combined = scores(set, outcome, Search::Combined, &keep);
        if combined.queries == 0 {
            continue;
        }
        let words = scores(set, outcome, Search::Words, &keep);
        let meaning = scores(set, outcome, Search::Meaning, &keep);
        let _ = writeln!(
            out,
            "| {name} | {} | {} | {} | {} | {} | {} |",
            combined.queries,
            percent(combined.recall),
            percent(combined.mrr),
            percent(combined.ndcg),
            percent(words.recall),
            percent(meaning.recall)
        );
    }
    out
}

/// One line per configuration, for comparing them.
pub fn summary(set: &EvalSet, outcomes: &[Outcome]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "| Model | Passage tokens | Passages | Combined recall@10 | Combined nDCG@10 \
         | Meaning recall@10 | Cross-language, meaning recall@10 | Arabic documents, combined recall@10 \
         | Latin-script documents, combined recall@10 | Passages a second |"
    );
    let _ = writeln!(
        out,
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    );
    for outcome in outcomes {
        let all = |search| scores(set, outcome, search, |_| true);
        let cross = scores(set, outcome, Search::Meaning, |q| {
            q.kind == Kind::CrossLanguage
        });
        let arabic = scores(set, outcome, Search::Combined, |q| q.doc_language == "ar");
        let latin = scores(set, outcome, Search::Combined, |q| q.doc_language != "ar");
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.1} |",
            outcome.config.model.name,
            outcome.config.tokens,
            outcome.passages,
            percent(all(Search::Combined).recall),
            percent(all(Search::Combined).ndcg),
            percent(all(Search::Meaning).recall),
            percent(cross.recall),
            percent(arabic.recall),
            percent(latin.recall),
            outcome.passages_per_second()
        );
    }
    out
}

fn percent(score: f64) -> String {
    format!("{:.1}", 100.0 * score)
}

/// Compare scores with the minimums in `thresholds`, one per line:
/// `<search> <group> <metric> <minimum>`, such as
/// `combined all recall@10 0.85`. Returns the lines that failed.
pub fn check(set: &EvalSet, outcome: &Outcome, thresholds: &str) -> Result<Vec<String>> {
    let mut failed = Vec::new();
    for (number, line) in thresholds.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [search, group_name, metric, minimum] = fields[..] else {
            bail!("thresholds line {}: expected 4 fields", number + 1);
        };
        let search = Search::ALL
            .into_iter()
            .find(|s| s.name() == search)
            .with_context(|| format!("thresholds line {}: unknown search {search}", number + 1))?;
        let minimum: f64 = minimum.parse()?;
        let scores = scores(set, outcome, search, group(group_name)?);
        let value = match metric {
            "recall@10" => scores.recall,
            "mrr@10" => scores.mrr,
            "ndcg@10" => scores.ndcg,
            _ => bail!("thresholds line {}: unknown metric {metric}", number + 1),
        };
        let verdict = if value + 1e-9 >= minimum {
            "ok"
        } else {
            "BELOW"
        };
        println!(
            "{verdict:>5}  {search:?} {group_name} {metric}: {value:.3} (minimum {minimum:.3})"
        );
        if verdict == "BELOW" {
            failed.push(line.to_string());
        }
    }
    Ok(failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Judged;
    use crate::run::{Config, Outcome};
    use std::collections::HashMap;

    fn query(kind: Kind) -> Query {
        Query {
            source: Source::Domain,
            kind,
            language: "en".into(),
            text: "q".into(),
            doc: "d.txt".into(),
            doc_language: "en".into(),
            line: 1,
            answer: "a".into(),
        }
    }

    /// Two queries: the exact one is found first, the descriptive one not at all.
    fn example() -> (EvalSet, Outcome) {
        let set = EvalSet {
            docs: Vec::new(),
            queries: vec![query(Kind::Exact), query(Kind::Descriptive)],
        };
        let judged = |hit: bool| {
            Search::ALL
                .into_iter()
                .map(|search| {
                    let relevant = vec![hit];
                    (search, Judged { relevant, total: 1 })
                })
                .collect::<HashMap<_, _>>()
        };
        let outcome = Outcome {
            config: Config {
                model: catchword_embed::GRANITE_97M,
                tokens: 350,
                overlap: 50,
                candidates: 50,
            },
            passages: 1,
            load_seconds: 1.0,
            embed_seconds: 1.0,
            query_seconds: 1.0,
            judged: vec![judged(true), judged(false)],
        };
        (set, outcome)
    }

    #[test]
    fn the_check_reports_every_threshold_that_is_not_met() {
        let (set, outcome) = example();
        let thresholds = "# comment\n\
                          combined kind:exact recall@10 0.9\n\
                          combined all recall@10 0.6\n";
        let failed = check(&set, &outcome, thresholds).unwrap();
        // Exact: 1.0, met. All: 0.5, below 0.6.
        assert_eq!(failed, vec!["combined all recall@10 0.6".to_string()]);
    }

    #[test]
    fn a_malformed_threshold_is_an_error_not_a_pass() {
        let (set, outcome) = example();
        assert!(check(&set, &outcome, "combined all recall@10").is_err());
        assert!(check(&set, &outcome, "combined nowhere recall@10 0.5").is_err());
        assert!(check(&set, &outcome, "combined all speed 0.5").is_err());
    }
}
