use clap::Parser;
use log::{error, info, LevelFilter};
use rayon::prelude::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use simulator::{Report, SimBuilder};

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
    run: u64,
    #[arg(long = "graph-source", short = 'g', default_value = "lnd")]
    graph_type: network_parser::GraphSource,
    /// Number of src/dest pairs to use in the simulation
    #[arg(long = "payments", short = 'p', default_value_t = 1000)]
    num_pairs: usize,
    /// The number of adversarial ASs to simulate (top-n)
    #[arg(long = "num-as", short = 'n', default_value_t = 5)]
    num_adv_as: usize,
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
        "Simulation results will be written to {:#?}/ directory.",
        output_dir
    );
    let amounts = if let Some(amount) = args.amount {
        vec![amount]
    } else {
        vec![100, 1000, 10000, 100000, 1000000, 10000000]
    };
    let mut sim_report = Report(args.run, vec![]);
    let results = Arc::new(Mutex::new(Vec::with_capacity(amounts.len())));
    amounts.par_iter().for_each(|amount| {
        info!("Starting simulation for {amount} sat.");
        let msat = simlib::to_millisatoshi(*amount);
        let mut builder = SimBuilder::new(args.run, &graph, msat, args.num_pairs, args.num_adv_as);
        let sim_output = builder.simulate();
        results.lock().unwrap().push(sim_output);
        info!("Completed simulation for {amount} sat.");
    });
    if let Ok(s) = results.lock() {
        sim_report.1 = s.clone();
    }
    sim_report
        .write_to_file(output_dir)
        .expect("Failed to write report to file.");
}
