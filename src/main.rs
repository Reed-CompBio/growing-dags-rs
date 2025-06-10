use std::path::PathBuf;

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

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Files {
        /// The tab-separated interactome, without a header, containing (a, b) := a -> b directed pairs
        /// with weights - e.g. `SOME_NODE_A\tSOME_NODE_B\t0.683`
        interactome: PathBuf,
        /// The tab-separated initial DAG, which is usually a known gold-standard pathway in the above PPI.
        dag: PathBuf,
        /// The sources Growing DAGs should try to start at.
        sources: PathBuf,
        /// The targets Growing DAGs should try to end at.
        targets: PathBuf,
    },
    Folder {
        path: PathBuf
    }
}

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init_timed();

    let cli = Cli::parse();

    match cli.command {
        Commands::Folder { path } => {
            let interactome = path.join("interactome.txt");
            let dag = path.join("dag.txt");
            let sources = path.join("sources.txt");
            let targets = path.join("targets.txt");
            handle_files(interactome, dag, sources, targets, cli.no_log_transform, cli.k)
        },
        Commands::Files { interactome, dag, sources, targets } => {
            handle_files(interactome, dag, sources, targets, cli.no_log_transform, cli.k)
        }
    }
}

fn handle_files(
    interactome: PathBuf,
    dag: PathBuf,
    sources: PathBuf,
    targets: PathBuf,
    no_log_transform: bool,
    k: usize,
) -> anyhow::Result<()> {
    info!("Reading sources & targets...");
    let sources = read_lines(&sources)?;
    let targets = read_lines(&targets)?;

    info!("Caching interactome...");
    let network = if no_log_transform {
        Network::from_file::<LogWeightDataFactory>(&interactome)?
    } else {
        Network::from_file::<WeightDataFactory>(&interactome)?
    };

    info!("Preprocessing interactome...");
    let interactome = Interactome::attach_sources_and_targets(network, &sources, &targets, true)?;

    let mut dag = PartialDag::new(
        Network::<(), Never>::from_file_using_id_map::<EmptyTupleDataFactory>(
            &dag,
            &interactome.inner_network.id_map,
        )?,
        &sources,
        &targets,
    )?;

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
                    .map(|node| interactome.inner_network.id_from_idx(node).cloned().unwrap())
                    .collect::<Vec<_>>()
                    .join("|");
                println!("{i}\t{weight}\t{path}");
            },
            None => {
                log::info!("No more paths could be constructed. Stopping at iteration {i}.");
                break;
            }
        }
    }

    Ok(())
}
