use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::anyhow;

use either::Either;
use growing_dags::parsing::interactome::Interactome;
use growing_dags::parsing::{
    dag::PartialDag,
    data::EmptyTupleDataFactory,
    weight::{LogWeightDataFactory, WeightDataFactory},
};
use growing_dags::{
    alg::{
        cost::EdgeCost,
        grow::{grow, GrowthCache},
    },
    util::read_lines,
};

use clap::{ArgAction, Parser, Subcommand};
use growing_dags::parsing::network::Network;
use log::*;
use never::Never;
use petgraph::algo::astar;
use petgraph::prelude::DiGraphMap;

extern crate pretty_env_logger;

#[derive(Parser)]
struct Cli {
    /// Whether to _not_ transform all the weights if they currently represent "higher = better."
    /// If your interactome already comes with weights that represent "lower = better,"
    /// use this option.
    #[arg(short, long, action=ArgAction::SetFalse)]
    no_log_transform: bool,

    /// The number of times to grow a new DAG.
    #[arg(short, long)]
    k: usize,

    /// The output file to use. By default,
    /// the output will print to stdout
    out: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Specify input through the paths of four files.
    Files {
        /// The tab-separated interactome, without a header, containing (a, b) := a -> b directed pairs
        /// with weights - e.g. `SOME_NODE_A\tSOME_NODE_B\t0.683`
        #[arg(short, long)]
        interactome: PathBuf,
        /// The tab-separated initial DAG, which is usually a known gold-standard pathway in the above PPI.
        /// If a dag isn't specified, one is automatically inferred through an arbitrarily chosen
        /// shortest path from any source to any target.
        #[arg(short, long)]
        dag: Option<PathBuf>,
        /// The sources Growing DAGs should try to start at.
        #[arg(short, long)]
        sources: PathBuf,
        /// The targets Growing DAGs should try to end at.
        #[arg(short, long)]
        targets: PathBuf,
    },
    /// Specify input through a single, containing folder.
    /// This is mainly for testing convenience.
    Folder {
        /// The folder containing an interactome.txt, dag.txt, sources.txt, and targets.txt.
        path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init_timed();

    let cli = Cli::parse();

    // https://stackoverflow.com/a/42216134/7589775
    let mut out_stream = if let Some(out) = cli.out {
        Box::new(File::open(out).unwrap()) as Box<dyn Write>
    } else {
        Box::new(std::io::stdout()) as Box<dyn Write>
    };

    match cli.command {
        Commands::Folder { path } => {
            let interactome = path.join("interactome.txt");
            let dag = path.join("dag.txt");
            let sources = path.join("sources.txt");
            let targets = path.join("targets.txt");
            handle_files(
                interactome,
                Some(dag),
                sources,
                targets,
                cli.no_log_transform,
                cli.k,
                &mut out_stream,
            )
        }
        Commands::Files {
            interactome,
            dag,
            sources,
            targets,
        } => handle_files(
            interactome,
            dag,
            sources,
            targets,
            cli.no_log_transform,
            cli.k,
            &mut out_stream,
        ),
    }
}

fn handle_files(
    interactome: PathBuf,
    dag: Option<PathBuf>,
    sources: PathBuf,
    targets: PathBuf,
    no_log_transform: bool,
    k: usize,
    mut out_stream: impl Write,
) -> anyhow::Result<()> {
    info!("Reading sources & targets...");
    let sources = read_lines(&sources)?;
    let targets = read_lines(&targets)?;

    if sources.len() == 0 {
        return Err(anyhow!("There must be at least one source."));
    }

    if targets.len() == 0 {
        return Err(anyhow!("There must be at least one target."));
    }

    info!("Caching interactome...");
    let network = if no_log_transform {
        Network::from_file::<LogWeightDataFactory>(&interactome)?
    } else {
        Network::from_file::<WeightDataFactory>(&interactome)?
    };

    info!("Preprocessing interactome...");
    let interactome = Interactome::attach_sources_and_targets(network, &sources, &targets, true)?;

    let dag = dag.filter(|dag| dag.exists());

    let mut dag = if let Some(dag) = dag {
        PartialDag::new(
            Network::<(), Never>::from_file_using_id_map::<EmptyTupleDataFactory>(
                &dag,
                &interactome.inner_network.id_map,
            )?,
            &sources,
            &targets,
        )?
    } else {
        // If no initial DAG is provided,
        // we get the shortest path from the first provided source to
        // the first provided target.
        let first_source = &sources[0];
        let first_target = &targets[0];

        let mapped_first_source = interactome
            .inner_network
            .id_map
            .get_by_left(first_source)
            .unwrap();
        let mapped_first_target = interactome
            .inner_network
            .id_map
            .get_by_left(first_target)
            .unwrap();

        let (_, shortest_path) = astar(
            &interactome.inner_network.graph,
            Either::Left(*mapped_first_source),
            |x| x == Either::Left(*mapped_first_target),
            |edge| edge.2 .0,
            |_| 0f64,
        )
        .expect("There should be shortest paths from all sources to all targets!");

        let mut graph = DiGraphMap::new();
        for edge in shortest_path.windows(2) {
            let source = edge[0]
                .map_right(|_| panic!("No node in the shortest path should be a super node."));
            let target = edge[1]
                .map_right(|_| panic!("No node in the shortest path should be a super node."));

            graph.add_edge(source, target, ());
        }

        let network = Network::new(graph, interactome.inner_network.id_map.clone());

        PartialDag::new(network, &sources, &targets)?
    };

    info!("Preparing cache...");
    let inner_interactome = interactome.clone();

    for i in 1..=k {
        info!("Growing DAGs: iteration {i}.");
        let mut cache = GrowthCache::new(inner_interactome.clone());
        match grow(&interactome, &mut dag, &mut cache, &mut EdgeCost)? {
            Some((weight, path)) => {
                let path = path
                    .into_iter()
                    .filter_map(|node| node.left())
                    .map(|node| {
                        interactome
                            .inner_network
                            .id_from_idx(node)
                            .cloned()
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
                    .join("|");
                writeln!(out_stream, "{i}\t{weight}\t{path}")?;
            }
            None => {
                log::warn!("No more paths could be constructed. Stopping at iteration {i}.");
                break;
            }
        }
    }

    Ok(())
}
