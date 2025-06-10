use std::{
    cmp::{Ordering, Reverse},
    collections::{hash_map::Entry, BinaryHeap, HashMap}, hash::Hash,
};

use ordered_float::OrderedFloat;
use petgraph::{
    prelude::DiGraphMap,
    visit::{EdgeRef, VisitMap, Visitable},
};
use xxhash_rust::xxh3::Xxh3Builder;

use crate::parsing::{network::NetworkIndexError, weight::Weight};

/// (source, target), (score, parent)
pub type Paths<V> = HashMap<(V, V), (f64, Option<V>)>;

/// For use in `BinaryHeap.` Stores a score and a scored object,
/// and is used in conjunction with `Reverse`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ScoreObject<K, T>(pub K, pub T);

impl<K: PartialOrd, T: PartialEq> PartialOrd for ScoreObject<K, T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        K::partial_cmp(&self.0, &other.0)
    }
}

impl<K: Ord, T: Eq> Ord for ScoreObject<K, T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        K::cmp(&self.0, &other.0)
    }
}

pub fn calculate_paths<V: Clone + Copy + Eq + Ord + Hash>(
    paths: &mut Paths<V>,
    graph: &DiGraphMap<V, Weight, Xxh3Builder>,
    source: V,
    targets: &[V],
    ignore: &[V],
) -> Result<(), NetworkIndexError> {
    // we reimplement this from
    // https://docs.rs/petgraph/0.8.2/src/petgraph/algo/dijkstra.rs.html#88-138
    // adjusted with the heuristics from Growing DAGs supplements.

    // TODO(perf): this might be bad for perf? we will see in the benchmarks.
    let mut targets = targets.to_vec();

    let mut visited = graph.visit_map();
    let mut visit_next = BinaryHeap::new();

    paths.insert((source, source), (0_f64, None));
    visit_next.push(Reverse(ScoreObject(OrderedFloat(0_f64), source)));
    while let Some(Reverse(ScoreObject(node_score, node))) = visit_next.pop() {
        if visited.is_visited(&node) {
            continue;
        }

        if let Some(idx) = targets.iter().position(|target| *target == node) {
            targets.remove(idx);
            if targets.is_empty() {
                return Ok(());
            }
        }

        if ignore.contains(&node) {
            continue;
        }

        for edge in graph.edges(node) {
            let next = edge.target();
            if visited.is_visited(&next) {
                continue;
            }

            let next_score = node_score + edge.weight().0;
            match paths.entry((source, next)) {
                Entry::Occupied(ent) => {
                    if next_score.0 < ent.get().0 {
                        *ent.into_mut() = (*next_score, Some(node));
                        visit_next.push(Reverse(ScoreObject(next_score, next)));
                    }
                }
                Entry::Vacant(ent) => {
                    ent.insert((*next_score, Some(node)));
                    visit_next.push(Reverse(ScoreObject(next_score, next)));
                }
            }
        }
        visited.visit(node);
    }

    Ok(())
}
