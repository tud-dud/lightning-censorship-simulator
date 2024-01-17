use clap::Parser;
use log::{error, info, LevelFilter};
use simlib::{PaymentParts, RoutingMetric, Simulation};
use std::path::PathBuf;

#[derive(clap::Parser)]
#[command(name = "simulator", version, about)]
struct Cli {
    /// Path to JSON ile describing topology
    graph_file: PathBuf,
    #[arg(long = "log", short = 'l', default_value = "info")]
    log_level: LevelFilter,
    /// Path to directory in which the results will be stored
    #[arg(long = "out", short = 'o')]
    output_dir: Option<PathBuf>,
    /// The payment volume (in sat) we are trying to route
    #[arg(long = "amount", short = 'a')]
    amount: Option<usize>,
    /// Set the seed for the simulation
    #[arg(long, short, default_value_t = 19)]
    _run: u64,
    #[arg(long = "graph-source", default_value = "lnd")]
    graph_type: network_parser::GraphSource,
    verbose: bool,
}

fn main() {
    let args = Cli::parse();
    let log_level = args.log_level;
    env_logger::builder().filter_level(log_level).init();
    let graph_source = args.graph_type;
    let g = network_parser::Graph::from_json_file(
        std::path::Path::new(&args.graph_file),
        graph_source.clone(),
    );
    let graph = match g {
        Ok(graph) => simlib::core_types::graph::Graph::to_sim_graph(&graph, graph_source),
        Err(e) => {
            error!("Error in graph file {}. Exiting.", e);
            std::process::exit(-1)
        }
    };
    let output_dir = if let Some(output_dir) = args.output_dir {
        output_dir
    } else {
        PathBuf::from("sim-results")
    };
    info!(
        "Graph metrics will be written to {:#?}/ directory.",
        output_dir
    );
    let nodes_wo_address: f32 = graph
        .get_nodes()
        .iter()
        .map(|n| n.addresses.len())
        .filter(|n| *n < 1)
        .count() as f32;
    info!(
        "{}% of nodes without a network address",
        (nodes_wo_address / graph.node_count() as f32) * 100.0
    );
    let amounts = if let Some(amount) = args.amount {
        vec![amount]
    } else {
        vec![1000, 10000, 100000, 1000000, 10000000]
    };
    //let results = Vec::with_capacity(amounts.len());
    amounts.iter().for_each(|amount| {
        info!("Starting simulation for {amount} sat.");
        let amount = simlib::to_millisatoshi(*amount);
        let _sim = Simulation::new(
            0,
            graph.clone(),
            amount,
            RoutingMetric::MinFee,
            PaymentParts::Split,
            None,
            &[],
        );
        info!("Completed simulation for {amount} sat.");
    });
    info!("Starting simulation..");
}
