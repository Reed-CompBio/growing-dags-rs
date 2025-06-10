use std::cmp::Ordering;

use either::Either;
use never::Never;
use petgraph::Direction;
use thiserror::Error;

use crate::parsing::network::Network;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SuperNode {
    Source,
    Target
}

impl Ord for SuperNode {
    fn cmp(&self, other: &Self) -> Ordering {
        if self == other {
            Ordering::Equal
        } else if *self == SuperNode::Source {
            // source is less than target
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}

impl PartialOrd for SuperNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug)]
pub struct Interactome<E> {
    pub inner_network: Network<E, SuperNode>,

    pub sources: Vec<usize>,
    pub targets: Vec<usize>,
}

#[derive(Debug, Error)]
pub enum InteractomeAttachError {
    #[error("Source '{0}' does not exist in the interactome.")]
    SourceNotExists(String),
    #[error("Target '{0}' does not exist in the interactome.")]
    TargetNotExists(String),
}

impl<E: Default + Clone> Interactome<E> {
    pub fn attach_sources_and_targets(
        network: Network<E, Never>,
        sources: &[String],
        targets: &[String],
        require_sources_and_targets: bool
    ) -> Result<Self, InteractomeAttachError> {
        let mut network = network.cast_over_never();
        let super_source = network.graph.add_node(Either::Right(SuperNode::Source));
        let super_target = network.graph.add_node(Either::Right(SuperNode::Target));

        network
            .prune(sources, Direction::Incoming, require_sources_and_targets)
            .map_err(|err| InteractomeAttachError::SourceNotExists(err.0))?;
        network
            .prune(targets, Direction::Outgoing, require_sources_and_targets)
            .map_err(|err| InteractomeAttachError::TargetNotExists(err.0))?;

        let sources = sources
            .iter()
            .filter_map(|source| {
                let source_id = network
                    .id_map
                    .get_by_left(source)
                    .copied();

                if !require_sources_and_targets && source_id.is_none() {
                    return None;
                }
                
                Some(source_id.ok_or_else(|| InteractomeAttachError::SourceNotExists(source.to_string())))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let targets = targets
            .iter()
            .filter_map(|target| {
                let target_id = network
                    .id_map
                    .get_by_left(target)
                    .copied();

                if !require_sources_and_targets && target_id.is_none() {
                    return None;
                }
                
                Some(target_id.ok_or_else(|| InteractomeAttachError::SourceNotExists(target.to_string())))
            })
            .collect::<Result<Vec<_>, _>>()?;

        for source_id in &sources {
            network
                .graph
                .add_edge(super_source, Either::Left(*source_id), E::default());
        }

        for target_id in &targets {
            network
                .graph
                .add_edge(Either::Left(*target_id), super_target, E::default());
        }

        Ok(Self {
            inner_network: network,
            sources,
            targets
        })
    }

    /// Gets a pretty-printed name of the string from a node index.
    pub fn name_from_idx(&self, id: Either<usize, SuperNode>) -> Option<String> {
        match id {
            Either::Left(id) => self.inner_network.id_from_idx(id).cloned(),
            Either::Right(SuperNode::Source) => Some("[[Super Source]]".to_string()),
            Either::Right(SuperNode::Target) => Some("[[Super Target]]".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use petgraph::visit::IntoEdgeReferences;

    use crate::parsing::weight::WeightDataFactory;

    use super::*;

    #[test]
    fn attach_works() {
        let network = Network::from_lines::<WeightDataFactory, _>(
            vec![
                Ok("A\t1\t0.123".to_string()),
                Ok("B\t1\t0.123".to_string()),
                Ok("C\t2\t0.123".to_string()),
                Ok("K\tC\t0.123".to_string()), // this edge will be ignored
                Ok("1\t3\t0.123".to_string()),
                Ok("2\t3\t0.123".to_string()),
                Ok("3\tX\t0.123".to_string()),
                Ok("3\tY\t0.123".to_string()),
            ]
            .into_iter(),
        )
        .unwrap();

        let interactome = Interactome::attach_sources_and_targets(
            network,
            &["A".to_string(), "B".to_string(), "C".to_string()],
            &["X".to_string(), "Y".to_string()],
            true
        )
        .unwrap();

        assert_eq!(
            interactome
                .inner_network
                .graph
                .edge_references()
                .collect::<Vec<_>>()
                .len(),
            7 + 3 + 2
        );
    }
}
