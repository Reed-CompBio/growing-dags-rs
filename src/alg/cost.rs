use either::Either;
use petgraph::algo::all_simple_paths;
use xxhash_rust::xxh3::Xxh3Builder;

use crate::parsing::{dag::PartialDag, interactome::{Interactome, SuperNode}, weight::Weight};

/// A 'cost' trait. This trait is usually added to some cost cache,
/// allowing for persistent information across `cost_of` runs.
pub trait Cost {
    /// The cost of a certain Path (`nodes`) being added to an existing DAG,
    /// passed with the weights of the main (not candidate) interactome.
    ///
    /// This is called 'relative cost,' as, we don't want to actually get the full cost - instead,
    /// we only care about the cost of the new path
    fn relative_cost_of(
        &mut self,
        main: &Interactome<Weight>,
        dag: &PartialDag<()>,
        nodes: &[Either<usize, SuperNode>],
    ) -> f64;
}

/// The **min edge cost** function: we simply minimize
/// the weights across all of the edges.
#[derive(Debug, Default, Clone, Copy)]
pub struct EdgeCost;

impl Cost for EdgeCost {
    fn relative_cost_of(
        &mut self,
        main: &Interactome<Weight>,
        dag: &PartialDag<()>,
        nodes: &[Either<usize, SuperNode>],
    ) -> f64 {
        // Compute additional cost
        let mut added_cost = 0_f64;
        for i in 0..nodes.len() - 1 {
            let source = nodes[i];
            let target = nodes[i + 1];
            if !dag.0.inner_network.graph.contains_edge(source, target) {
                // DAG does not have the edge - add it to the cost
                let weight = main
                    .inner_network
                    .graph
                    .edge_weight(source, target)
                    .unwrap_or_else(|| panic!("DAG should be a subgraph of the main interactome. Instead, found {source:?}, {target:?}"));
                added_cost += weight.0;
            }
        }

        added_cost
    }
}

/// The **min paths cost** function: we try to minimize
/// the weights of every single path provided in the new DAG.
pub struct PathCost;

impl Cost for PathCost {
    fn relative_cost_of(
        &mut self,
        main: &Interactome<Weight>,
        dag: &PartialDag<()>,
        nodes: &[Either<usize, SuperNode>],
    ) -> f64 {
        let mut new_dag = dag.clone();

        for i in 0..nodes.len() - 1 {
            let source = nodes[i];
            let target = nodes[i + 1];

            new_dag.0.inner_network.graph.add_edge(source, target, ());
        }

        let paths = all_simple_paths::<Vec<_>, _, Xxh3Builder>(
            &new_dag.0.inner_network.graph,
            Either::Right(SuperNode::Source),
            Either::Right(SuperNode::Target),
            0, None
        );

        let mut relative_cost = 0_f64;

        for path in paths {
            for i in 0..path.len() - 1 {
                let source = path[i];
                let target = path[i + 1];
                let weight = main.inner_network.graph.edge_weight(source, target).copied().unwrap();
                relative_cost += weight.0;
            }
        }

        relative_cost
    }
}

#[cfg(test)]
mod tests {
    use crate::parsing::{
        data::EmptyTupleDataFactory, network::Network, weight::WeightDataFactory,
    };

    use super::*;

    #[test]
    fn test_edge_cost() {
        let main_network = Network::from_lines::<WeightDataFactory, _>(
            vec![
                Ok("A\tB\t0.5".to_string()),
                Ok("B\tC\t0.5".to_string()),
                Ok("B\tD\t0.5".to_string()),
                Ok("D\tC\t0.7".to_string()),
            ]
            .into_iter(),
        )
        .unwrap();
        let main_network_id_map = main_network.id_map.clone();

        assert!(main_network_id_map.contains_left("A"));
        assert!(main_network_id_map.contains_left("B"));
        assert!(main_network_id_map.contains_left("C"));
        assert!(main_network_id_map.contains_left("D"));

        let interactome = Interactome::attach_sources_and_targets(
            main_network,
            &["A".to_string()],
            &["C".to_string()],
            true
        )
        .unwrap();

        assert_eq!(
            interactome.inner_network.as_nodes(&["B", "D"]).unwrap(),
            vec![Either::Left(1), Either::Left(3)]
        );

        let dag = {
            let dag_network = Network::from_lines_using_id_map::<EmptyTupleDataFactory, _>(
                vec![Ok("A\tB".to_string()), Ok("B\tC".to_string())].into_iter(),
                &main_network_id_map,
            )
            .unwrap();

            PartialDag::new(dag_network, &["A".to_string()], &["C".to_string()]).unwrap()
        };

        let mut edge_cost = EdgeCost;

        let cost = edge_cost.relative_cost_of(
            &interactome,
            &dag,
            &interactome.inner_network.as_nodes(&["B", "D"]).unwrap(),
        );
        assert_eq!(cost, 0.5);

        let cost = edge_cost.relative_cost_of(
            &interactome,
            &dag,
            &interactome
                .inner_network
                .as_nodes(&["B", "D", "C"])
                .unwrap(),
        );
        assert_eq!(cost, 1.2);
    }
}
