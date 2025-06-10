use std::collections::HashMap;

use either::Either;
use petgraph::{algo::toposort, visit::IntoEdgeReferences};

use crate::{
    alg::path::calculate_paths,
    parsing::{
        dag::PartialDag,
        interactome::{Interactome, SuperNode},
        network::{Network, NetworkIndexError},
        weight::Weight,
    },
    util::get_ancestors,
};

use super::{cost::Cost, path::Paths};

pub struct GrowthCache {
    candidate: Network<Weight, SuperNode>,
}

impl GrowthCache {
    pub fn new(interactome: Interactome<Weight>) -> Self {
        Self {
            candidate: interactome.inner_network,
        }
    }
}

/// **The heart of Growing DAGs**.
/// We assume that interactome and DAG have the same
/// underlying id_map.
pub fn produce_dag<C: Cost>(
    interactome: &Interactome<Weight>,
    dag: &PartialDag<()>,
    cache: &mut GrowthCache,
    cost: &mut C,
) -> Result<Option<(f64, Vec<Either<usize, SuperNode>>)>, NetworkIndexError> {
    // Prepare the candidate graph by removing the current DAG's edges
    for (source_idx, target_idx, _) in dag.0.inner_network.graph.edge_references() {
        cache.candidate.graph.remove_edge(source_idx, target_idx);
        
        // Remove empty vertices along the edges, getting the induced edge graph
        // of the candidate (except for vertices on the candidate who were already alone)
        // TODO: is that okay?
        if cache.candidate.is_node_empty(source_idx) {
            cache.candidate.graph.remove_node(source_idx);
        }
        if cache.candidate.is_node_empty(target_idx) {
            cache.candidate.graph.remove_node(target_idx);
        }
    }

    // Prepare our 'parents' dictionary of (source, target) <-> (cost, parent)
    let mut paths_parents: Paths<Either<usize, SuperNode>> = HashMap::new();
    let mut all_targets: HashMap<Either<usize, SuperNode>, Vec<Either<usize, SuperNode>>> = HashMap::new();

    // Create a topological sorting of all of the current nodes
    let nodes = toposort(&dag.0.inner_network.graph, None).unwrap();

    // Re-iterate over every single existing node in the DAG, preparing our distance cache for later cost-minimization.
    for (idx, node_id) in nodes.into_iter().enumerate() {
        let node_name = dag.0.name_from_idx(node_id).unwrap();
        log::trace!("On the DAG node {node_name}.");

        if dag
            .0
            .inner_network
            .graph
            .contains_edge(node_id, Either::Right(SuperNode::Target))
        {
            paths_parents.insert((node_id, Either::Right(SuperNode::Target)), (f64::INFINITY, None));
            log::trace!("Node ID {node_id:?} is connected to the super target. Adjusting.");
            continue;
        }

        if !cache.candidate.graph.contains_node(node_id) {
            log::debug!("Skipping {node_id:?} named {node_name} as it is not in the candidate graph.");
            continue;
        }

        let ancestors = get_ancestors(&dag.0.inner_network.graph, node_id);

        // Preprocess the candidate graph by removing all ancestors of the current node
        for ancestor in &ancestors {
            cache.candidate.graph.remove_node(*ancestor);
            log::trace!("Removing ancestor {ancestor:?}");
        }

        // targets are the incomparable elements and the descendents of the DAG.
        // first, collect only the nodes which are not the ancestors or are not the current node
        let targets = dag
            .0
            .inner_network
            .graph
            .nodes()
            .filter(|&n| n != node_id && !ancestors.contains(&n))
            .collect::<Vec<_>>();

        log::info!("Running dijkstra on {node_name} ({}/{}) over {} edges", idx, dag.0.inner_network.graph.node_count(), &cache.candidate.graph.edge_count());
        // and calculate paths!
        calculate_paths(
            &mut paths_parents,
            &cache.candidate.graph,
            node_id,
            &targets,
            &targets,
        )?;

        all_targets.insert(node_id, targets);
    }

    let paths = all_targets
        .into_iter()
        .flat_map(|(source, targets)| {
            let mut paths = Vec::with_capacity(targets.len());
            for target in targets {
                let mut path = vec![];
                let mut current_loop_parent = Some(target);
                while let Some(current_parent) = current_loop_parent {
                    path.push(current_parent);
                    current_loop_parent = paths_parents
                        .get(&(source, current_parent))
                        .and_then(|val| val.1)
                }
                if path.len() < 2 {
                    continue;
                }
                path.reverse();
                paths.push(path);
            }

            paths
        })
        .collect::<Vec<_>>();

    // Calculate the best possible path given the cost function.
    let best_path = paths.into_iter().min_by(|x, y| {
        cost.relative_cost_of(interactome, dag, x)
            .total_cmp(&cost.relative_cost_of(interactome, dag, y))
    });

    Ok(best_path.map(|best_path| {
        (
            cost.relative_cost_of(interactome, dag, &best_path),
            best_path,
        )
    }))
}

pub fn grow<C: Cost>(
    interactome: &Interactome<Weight>,
    dag: &mut PartialDag<()>,
    cache: &mut GrowthCache,
    cost: &mut C,
) -> Result<Option<(f64, Vec<Either<usize, SuperNode>>)>, NetworkIndexError> {
    // grab the best path
    let next_best_path = produce_dag(interactome, dag, cache, cost)?;

    // and add it to the DAG.
    if let Some((weight, next_best_path)) = next_best_path {
        log::info!("Writing a path of length {}", next_best_path.len());

        for i in 0..next_best_path.len() - 1 {
            let source_node = next_best_path[i];
            let target_node = next_best_path[i + 1];

            dag.0
                .inner_network
                .graph
                .add_edge(source_node, target_node, ());
        }

        return Ok(Some((weight, next_best_path)));
    }

    Ok(None)
}
