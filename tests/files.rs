use std::path::Path;

use either::Either;
use growing_dags::{
    alg::{
        cost::EdgeCost,
        grow::{grow, GrowthCache},
    },
    parsing::{
        dag::PartialDag,
        data::{DataFactory, EmptyTupleDataFactory},
        interactome::Interactome,
        network::Network,
        weight::{Weight, WeightDataFactory},
    },
    util::read_lines,
};
use never::Never;
use petgraph::visit::IntoEdgeReferences;

struct Fixture {
    interactome: Interactome<Weight>,
    dag: PartialDag<()>,
    sources: Vec<String>,
    targets: Vec<String>,
}

fn grab_fixture<F: DataFactory<Weight>>(folder: &Path) -> Fixture {
    let interactome_path = folder.join("interactome.txt");
    let dag_path = folder.join("dag.txt");
    let sources_path = folder.join("sources.txt");
    let targets_path = folder.join("targets.txt");

    let sources = read_lines(&sources_path).unwrap();
    let targets = read_lines(&targets_path).unwrap();

    let network = Network::from_file::<F>(&interactome_path).unwrap();
    let interactome = Interactome::attach_sources_and_targets(network, &sources, &targets, true).unwrap();

    let dag = PartialDag::new(
        Network::<(), Never>::from_file_using_id_map::<EmptyTupleDataFactory>(
            &dag_path,
            &interactome.inner_network.id_map,
        )
        .unwrap(),
        &sources,
        &targets,
    )
    .unwrap();

    Fixture {
        interactome,
        dag,
        sources,
        targets,
    }
}

#[ctor::ctor]
fn init() {
    pretty_env_logger::init();
}

#[test]
fn test_triangle() {
    let Fixture {
        interactome,
        mut dag,
        sources: _,
        targets: _,
    } = grab_fixture::<WeightDataFactory>(Path::new("./tests/fixtures/triangle"));

    // two extra edges for the super-source and the super-target
    assert_eq!(
        interactome
            .inner_network
            .graph
            .edge_references()
            .collect::<Vec<_>>()
            .len(),
        3 + 2
    );
    assert_eq!(
        dag.0
            .inner_network
            .graph
            .edge_references()
            .collect::<Vec<_>>()
            .len(),
        4
    );

    let mut cache = GrowthCache::new(interactome.clone());
    let grow_weight = grow(&interactome, &mut dag, &mut cache, &mut EdgeCost).unwrap();

    assert_eq!(grow_weight, Some((1.0, vec![Either::Left(1), Either::Left(2)])));
    assert_eq!(
        dag.0
            .inner_network
            .graph
            .edge_references()
            .collect::<Vec<_>>()
            .len(),
        3 + 2 // this doesn't post-process remove the super nodes.
    );
}
