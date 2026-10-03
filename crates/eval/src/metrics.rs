//! Retrieval scores (ADR-19). Relevance is yes or no.

/// The judged top results of one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    /// For each returned result, best first: is it relevant?
    pub relevant: Vec<bool>,
    /// How many relevant passages the whole index holds for this query.
    pub total: usize,
}

/// The number of results scored.
pub const K: usize = 10;

impl Judged {
    /// 1 when a relevant passage is in the top K, else 0.
    pub fn recall(&self) -> f64 {
        if self.relevant.iter().take(K).any(|&r| r) {
            1.0
        } else {
            0.0
        }
    }

    /// 1 / rank of the first relevant passage in the top K, else 0.
    pub fn reciprocal_rank(&self) -> f64 {
        self.relevant
            .iter()
            .take(K)
            .position(|&r| r)
            .map_or(0.0, |index| 1.0 / (index as f64 + 1.0))
    }

    /// The gain of this order against the best possible order of the
    /// query's relevant passages, each discounted by log2(rank + 1).
    pub fn ndcg(&self) -> f64 {
        let gain = |rank: usize| 1.0 / (rank as f64 + 1.0).log2();
        let actual: f64 = (1..=K)
            .zip(self.relevant.iter())
            .filter(|(_, &relevant)| relevant)
            .map(|(rank, _)| gain(rank))
            .sum();
        let best: f64 = (1..=self.total.min(K)).map(gain).sum();
        if best == 0.0 {
            0.0
        } else {
            actual / best
        }
    }
}

/// Average scores over a group of queries.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Scores {
    pub queries: usize,
    pub recall: f64,
    pub mrr: f64,
    pub ndcg: f64,
}

impl Scores {
    pub fn of<'a>(judged: impl IntoIterator<Item = &'a Judged>) -> Self {
        let mut scores = Scores::default();
        for one in judged {
            scores.queries += 1;
            scores.recall += one.recall();
            scores.mrr += one.reciprocal_rank();
            scores.ndcg += one.ndcg();
        }
        if scores.queries > 0 {
            let n = scores.queries as f64;
            scores.recall /= n;
            scores.mrr /= n;
            scores.ndcg /= n;
        }
        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judged(relevant: &[bool], total: usize) -> Judged {
        Judged {
            relevant: relevant.to_vec(),
            total,
        }
    }

    #[test]
    fn a_relevant_first_result_scores_one_everywhere() {
        let one = judged(&[true, false, false], 1);
        assert_eq!(
            (one.recall(), one.reciprocal_rank(), one.ndcg()),
            (1.0, 1.0, 1.0)
        );
    }

    #[test]
    fn a_relevant_third_result() {
        let one = judged(&[false, false, true], 1);
        assert_eq!(one.recall(), 1.0);
        assert!((one.reciprocal_rank() - 1.0 / 3.0).abs() < 1e-12);
        // log2(3 + 1) = 2.
        assert!((one.ndcg() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn nothing_relevant_in_the_top_ten_scores_zero() {
        let mut relevant = vec![false; 10];
        relevant.push(true);
        let one = judged(&relevant, 1);
        assert_eq!(
            (one.recall(), one.reciprocal_rank(), one.ndcg()),
            (0.0, 0.0, 0.0)
        );
        // No relevant passage in the index at all: also zero, not an error.
        assert_eq!(judged(&[false], 0).ndcg(), 0.0);
    }

    #[test]
    fn ndcg_compares_with_the_best_order_of_all_relevant_passages() {
        // Two relevant passages exist; only one was found, at rank 1.
        let one = judged(&[true, false], 2);
        let best = 1.0 + 1.0 / 3f64.log2();
        assert!((one.ndcg() - 1.0 / best).abs() < 1e-12);
    }

    #[test]
    fn scores_are_averages() {
        let all = [judged(&[true], 1), judged(&[false, true], 1)];
        let scores = Scores::of(&all);
        assert_eq!(scores.queries, 2);
        assert_eq!(scores.recall, 1.0);
        assert!((scores.mrr - 0.75).abs() < 1e-12);
    }
}
