use std::{
    fs::File,
    hash::{BuildHasher, Hash},
    io::{BufRead, BufReader},
    path::Path,
};

use petgraph::{
    prelude::GraphMap,
    visit::{Dfs, Reversed},
    Direction, EdgeType,
};

pub fn get_related<N: Copy + Hash + Ord, E, Ty: EdgeType, S: BuildHasher>(
    graph: &GraphMap<N, E, Ty, S>,
    node: N,
    direction: Direction,
) -> Vec<N> {
    let mut relators = vec![];

    let mut dfs = match direction {
        Direction::Incoming => Dfs::new(Reversed(&graph), node),
        Direction::Outgoing => Dfs::new(&graph, node),
    };

    while let Some(nx) = dfs.next(Reversed(&graph)) {
        if nx == node {
            continue;
        }

        relators.push(nx);
    }

    relators
}

/// Get some arbitrary list of ancestors in no particular order. This list does not
/// contain `node`.
pub fn get_ancestors<N: Copy + Hash + Ord, E, Ty: EdgeType, S: BuildHasher>(
    graph: &GraphMap<N, E, Ty, S>,
    node: N,
) -> Vec<N> {
    get_related(graph, node, Direction::Incoming)
}

pub fn get_descendents<N: Copy + Hash + Ord, E, Ty: EdgeType, S: BuildHasher>(
    graph: &GraphMap<N, E, Ty, S>,
    node: N,
) -> Vec<N> {
    get_related(graph, node, Direction::Outgoing)
}

pub fn read_lines(path: &Path) -> anyhow::Result<Vec<String>> {
    Ok(BufReader::new(File::open(path)?)
        .lines()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use petgraph::prelude::DiGraphMap;

    use super::*;

    #[test]
    fn ancestor_not_self() {
        let graph: DiGraphMap<u32, ()> = DiGraphMap::from_edges(&[(2, 1), (1, 0)]);

        assert_eq!(get_ancestors(&graph, 0), vec![1, 2]);
    }

    #[test]
    fn cycle() {
        let graph: DiGraphMap<u32, ()> = DiGraphMap::from_edges(&[(0, 1), (1, 2), (2, 0)]);
        assert_eq!(get_descendents(&graph, 0), vec![2, 1]);
    }
}
