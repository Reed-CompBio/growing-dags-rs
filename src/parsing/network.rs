//! Generic Network wrapper for Graphs.
//! Algorithms here aren't generally optimized, but are rather optimized
//! for Growing DAG's specific use-cases.

use bimap::BiHashMap;
use either::Either;
use never::Never;
use petgraph::{prelude::{DiGraphMap, GraphMap}, visit::IntoEdgeReferences, Direction};
use xxhash_rust::xxh3::Xxh3Builder;
use std::{
    cmp::max,
    fs::File,
    io::{self, BufRead, BufReader},
    path::Path,
    hash::Hash
};
use thiserror::Error;

use super::data::DataFactory;

#[derive(Error, Debug)]
pub enum NetworkParsingError {
    #[error(transparent)]
    Misc(#[from] io::Error),
    #[error(transparent)]
    ParseDataError(#[from] anyhow::Error),
    #[error("line '{0}' has component size {1}, but requires {2} components (first and second interactome, then {3})")]
    InvalidSizeError(usize, usize, usize, String),
    #[error("id factory couldn't produce {0} at line {1}.")]
    FactoryOut(String, usize),
}

#[derive(Debug, Error)]
#[error("Node {0} is not present in this network.")]
pub struct NetworkIndexError(pub String);

/// A network.
/// This is a wrapper struct around some directed graph
/// and an id map which mapes gene names to numeric ids, since post-processing of genome names
/// only happens once output is generated (which is not often.)
#[derive(Clone, Debug)]
pub struct Network<E, S: Eq + Hash> {
    /// Interactome representation, using usize ids
    pub graph: DiGraphMap<Either<usize, S>, E, Xxh3Builder>,
    /// The map of the original string ids to the usize ids -
    /// we do this since most of the time spent processing the interactome
    /// will not care about the actual strings.
    pub id_map: BiHashMap<String, usize>,
    /// The max-size id
    max_id: usize,
}

impl<E: Clone, S: Eq + Hash + Copy + Ord> Network<E, S> {
    pub fn from_lines_over_id_map<
        F: DataFactory<E>,
        I: Iterator<Item = Result<String, io::Error>>,
    >(
        interactome_lines: I,
        mut id_map: BiHashMap<String, usize>,
        id_factory: impl Fn(String, usize) -> Option<usize>,
    ) -> Result<Self, NetworkParsingError> {
        let mut graph = DiGraphMap::new();
        let mut max_id = 0;

        for (idx, line) in interactome_lines.enumerate() {
            let line = line?;

            // newlines
            if line.is_empty() {
                continue;
            }

            // and comments
            if line.starts_with("#") {
                continue;
            }

            let components = line.split('\t').collect::<Vec<_>>();
            if components.len() != 2 + F::len() {
                // Transform the line position to match the file.
                return Err(NetworkParsingError::InvalidSizeError(
                    idx + 1,
                    components.len(),
                    2 + F::len(),
                    F::err_str(),
                ));
            }

            let source_interactome_name = components[0];
            let target_interactome_name = components[1];
            let data = F::from_strs(
                idx,
                components
                    .into_iter()
                    .map(|s| s.to_string())
                    .skip(2)
                    .collect(),
            )
            .map_err(NetworkParsingError::ParseDataError)?;

            let source_interactome = id_map
                .get_by_left(source_interactome_name)
                .copied()
                .or_else(|| {
                    id_factory(source_interactome_name.to_string(), id_map.len()).inspect(|&idx| {
                        let _ = graph.add_node(Either::Left(idx));
                        id_map.insert(source_interactome_name.to_string(), idx);
                        max_id = max(idx, max_id);
                    })
                })
                .ok_or_else(|| {
                    NetworkParsingError::FactoryOut(
                        source_interactome_name.to_string(),
                        id_map.len(),
                    )
                })?;

            let target_interactome = id_map
                .get_by_left(target_interactome_name)
                .copied()
                .or_else(|| {
                    id_factory(target_interactome_name.to_string(), id_map.len()).inspect(|&idx| {
                        let _ = graph.add_node(Either::Left(idx));
                        id_map.insert(target_interactome_name.to_string(), idx);
                        max_id = max(idx, max_id);
                    })
                })
                .ok_or_else(|| {
                    NetworkParsingError::FactoryOut(
                        target_interactome_name.to_string(),
                        id_map.len(),
                    )
                })?;

            graph.add_edge(Either::Left(source_interactome), Either::Left(target_interactome), data);
        }

        Ok(Self {
            id_map,
            graph,
            max_id,
        })
    }

    pub fn from_lines_using_id_map<
        F: DataFactory<E>,
        I: Iterator<Item = Result<String, io::Error>>,
    >(
        interactome_lines: I,
        id_map: &BiHashMap<String, usize>,
    ) -> Result<Self, NetworkParsingError> {
        Self::from_lines_over_id_map::<F, _>(
            interactome_lines,
            BiHashMap::new(),
            |str_identifier, _| id_map.get_by_left(&str_identifier).copied(),
        )
    }

    pub fn from_lines<F: DataFactory<E>, I: Iterator<Item = Result<String, io::Error>>>(
        interactome_lines: I,
    ) -> Result<Self, NetworkParsingError> {
        Self::from_lines_over_id_map::<F, _>(interactome_lines, BiHashMap::new(), |_, idx| {
            Some(idx)
        })
    }

    pub fn from_file_over_id_map<F: DataFactory<E>>(
        interactome: &Path,
        id_map: BiHashMap<String, usize>,
        id_factory: impl Fn(String, usize) -> Option<usize>,
    ) -> Result<Self, NetworkParsingError> {
        let lines = BufReader::new(File::open(interactome)?).lines();

        Self::from_lines_over_id_map::<F, _>(lines, id_map, id_factory)
    }

    pub fn from_file_using_id_map<F: DataFactory<E>>(
        interactome: &Path,
        id_map: &BiHashMap<String, usize>,
    ) -> Result<Self, NetworkParsingError> {
        Self::from_file_over_id_map::<F>(interactome, BiHashMap::new(), |str_identifier, _| {
            id_map.get_by_left(&str_identifier).copied()
        })
    }

    pub fn from_file<F: DataFactory<E>>(interactome: &Path) -> Result<Self, NetworkParsingError> {
        Self::from_file_over_id_map::<F>(interactome, BiHashMap::new(), |_, idx| Some(idx))
    }

    /// Gets a node index from a string (gene). The inverse of `Self::id_from_idx`.
    pub fn get_node(&self, node: &str) -> Result<usize, NetworkIndexError> {
        self.id_map
            .get_by_left(node)
            .ok_or(NetworkIndexError(node.to_string()))
            .copied()
    }

    /// Allocates a new id for a node.
    /// Prefer this over `self.graph.add_node`.
    pub fn add_node(&mut self) -> usize {
        let _ = self.graph.add_node(Either::Left(self.max_id + 1));
        self.max_id += 1;
        self.max_id
    }

    pub fn as_nodes(&self, nodes: &[&str]) -> Result<Vec<Either<usize, S>>, NetworkIndexError> {
        nodes.iter().map(|node| self.get_node(node).map(Either::Left)).collect()
    }

    /// Gets a string (gene) from a node index. The inverse of `Self::get_node`.
    pub fn id_from_idx(&self, id: usize) -> Option<&String> {
        self.id_map.get_by_right(&id)
    }

    /// Removes edges {direction} from {nodes}. For example, remove
    /// incoming edges from source nodes. If you want to remove a set of nodes instead,
    /// look at `Self::remove`.
    pub fn prune(
        &mut self,
        nodes: &[String],
        direction: Direction,
        require_nodes: bool,
    ) -> Result<(), NetworkIndexError> {
        let mut pooled_edges = vec![];
        for node in nodes {
            let node_id = self.id_map.get_by_left(node);
            if !require_nodes && node_id.is_none() {
                continue;
            }

            let node_id = node_id.ok_or_else(|| NetworkIndexError(node.to_string()))?;

            for (a, b, _edge_idx) in self.graph.edges_directed(Either::Left(*node_id), direction) {
                pooled_edges.push((a, b));
            }
        }

        for (a, b) in pooled_edges {
            self.graph.remove_edge(a, b);
        }

        Ok(())
    }

    pub fn destroy_right_nodes(self) -> Network<E, Never> {
        let mut new_graph = DiGraphMap::with_capacity(self.graph.node_count(), self.graph.edge_count());

        for node in self.graph.nodes() {
            new_graph.add_node(Either::Left(node.left().unwrap()));
        }

        for (a, b, e) in self.graph.edge_references() {
            new_graph.add_edge(
                Either::Left(a.left().unwrap()),
                Either::Left(b.left().unwrap()),
                e.clone(),
            );
        }
        
        Network {
            graph: new_graph,
            id_map: self.id_map,
            max_id: self.max_id
        }
    }

    pub fn is_node_empty(&self, node: Either<usize, S>) -> bool {
        self.graph.neighbors_directed(node, Direction::Incoming).next().is_none() &&
            self.graph.neighbors_directed(node, Direction::Outgoing).next().is_none()
    }
}

impl<E: Clone> Network<E, Never> {
    pub fn cast_over_never<S: Eq + Hash + Copy + Ord>(self) -> Network<E, S> {
        let mut new_graph: GraphMap<Either<usize, _>, E, _, _> = DiGraphMap::with_capacity(self.graph.node_count(), self.graph.edge_count());

        for node in self.graph.nodes() {
            new_graph.add_node(Either::Left(node.left().unwrap()));
        }

        for (a, b, e) in self.graph.edge_references() {
            new_graph.add_edge(
                Either::Left(a.left().unwrap()),
                Either::Left(b.left().unwrap()),
                e.clone(),
            );
        }
        
        Network {
            graph: new_graph,
            id_map: self.id_map,
            max_id: self.max_id
        }
    }
}

#[cfg(test)]
mod tests {
    use petgraph::visit::IntoEdgeReferences;

    use crate::parsing::weight::WeightDataFactory;

    use super::*;

    #[test]
    fn from_lines_len_check_cycle() {
        let network = Network::<_, ()>::from_lines::<WeightDataFactory, _>(
            vec![
                Ok("A\tB\t0.5".to_string()),
                Ok("B\tC\t0.5".to_string()),
                Ok("B\tD\t0.5".to_string()),
                Ok("D\tC\t0.5".to_string()),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(network.graph.nodes().len(), 4);
        assert_eq!(network.graph.edge_references().collect::<Vec<_>>().len(), 4);
        assert_eq!(network.id_map.len(), 4);
    }
}
