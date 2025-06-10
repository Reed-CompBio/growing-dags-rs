use crate::parsing::network::Network;
use never::Never;
use petgraph::algo::is_cyclic_directed;
use thiserror::Error;

use super::interactome::{Interactome, InteractomeAttachError};

#[derive(Debug, Error)]
pub enum DAGCreationError {
    #[error(transparent)]
    InteractomeAttachError(#[from] InteractomeAttachError),
    #[error("The passed in DAG has cycles!")]
    IsCyclic,
}

/// A partial DAG.
/// Note that only a subgraph of the network is guaranteed to be a DAG,
/// but this subgraph can be empty.
#[derive(Clone, Debug)]
pub struct PartialDag<E>(pub Interactome<E>);

impl<E: Clone + Default> PartialDag<E> {
    pub fn new(
        network: Network<E, Never>,
        sources: &[String],
        targets: &[String],
    ) -> Result<Self, DAGCreationError> {
        let interactome = Interactome::attach_sources_and_targets(network, sources, targets, false)?;

        if is_cyclic_directed(&interactome.inner_network.graph) {
            return Err(DAGCreationError::IsCyclic);
        }

        Ok(PartialDag(interactome))
    }
}
