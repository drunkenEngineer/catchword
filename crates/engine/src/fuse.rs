//! Combining keyword and meaning results into one list (ADR-5).
//!
//! Reciprocal rank fusion: a result earns 1 / (K + rank) from each list it
//! appears in, where rank 1 is the top. Only ranks are used, never the raw
//! scores, because keyword scores and similarity scores are not comparable.
//! A result found both ways therefore rises above one found only one way.

use std::collections::HashMap;
use std::hash::Hash;

/// The usual constant. Larger values flatten the difference between ranks.
pub const K: f64 = 60.0;

/// How a result was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    Keyword,
    Meaning,
    Both,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fused<T> {
    pub item: T,
    pub score: f64,
    pub found: Found,
}

/// Merge two ranked lists, best first. Each list must be best first too.
/// Equal scores keep the order in which results first appear, keyword
/// results first.
pub fn fuse<T: Copy + Eq + Hash>(keyword: &[T], meaning: &[T]) -> Vec<Fused<T>> {
    let mut fused: Vec<Fused<T>> = Vec::new();
    let mut position: HashMap<T, usize> = HashMap::new();
    for (list, found) in [(keyword, Found::Keyword), (meaning, Found::Meaning)] {
        for (index, item) in list.iter().enumerate() {
            let share = 1.0 / (K + index as f64 + 1.0);
            match position.get(item) {
                Some(&at) => {
                    fused[at].score += share;
                    if fused[at].found != found {
                        fused[at].found = Found::Both;
                    }
                }
                None => {
                    position.insert(*item, fused.len());
                    fused.push(Fused {
                        item: *item,
                        score: share,
                        found,
                    });
                }
            }
        }
    }
    // A stable sort keeps first-appearance order for equal scores.
    fused.sort_by(|a, b| b.score.total_cmp(&a.score));
    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(fused: &[Fused<char>]) -> String {
        fused.iter().map(|f| f.item).collect()
    }

    #[test]
    fn found_both_ways_ranks_above_found_one_way() {
        // "c" is third by keyword and third by meaning; "a" and "x" are top
        // of one list each.
        let fused = fuse(&['a', 'b', 'c'], &['x', 'y', 'c']);
        assert_eq!(fused[0].item, 'c');
        assert_eq!(fused[0].found, Found::Both);
        let expected = 2.0 / (K + 3.0);
        assert!((fused[0].score - expected).abs() < 1e-12);
    }

    #[test]
    fn one_list_keeps_its_order() {
        let fused = fuse(&['a', 'b', 'c'], &[]);
        assert_eq!(order(&fused), "abc");
        assert!(fused.iter().all(|f| f.found == Found::Keyword));
        let fused = fuse(&[], &['x', 'y']);
        assert_eq!(order(&fused), "xy");
        assert!(fused.iter().all(|f| f.found == Found::Meaning));
    }

    #[test]
    fn equal_ranks_interleave_keyword_first() {
        let fused = fuse(&['a', 'b'], &['x', 'y']);
        assert_eq!(order(&fused), "axby");
    }

    #[test]
    fn nothing_found_is_an_empty_list() {
        assert!(fuse::<char>(&[], &[]).is_empty());
    }
}
