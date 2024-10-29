use clap::Parser;
use log::{error, info, warn, LevelFilter};
use rayon::prelude::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use simulator::{
    AsIpMap, AsSelectionStrategy, InvolvedAsns, PacketDropStrategy, PaymentsDist,
    PerStrategyResults, Report, SimBuilder, SimOutput, SimResult,
};

#[derive(clap::Parser)]
#[command(name = "simulator", version, about)]
struct Cli {
    /// Path to JSON file describing topology
    graph_file: PathBuf,
    #[arg(long = "log", short = 'l', default_value = "info")]
    log_level: LevelFilter,
    /// Path to directory where the results will be stored
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
    /// AS selection strategy. 0 for number of nodes and 1 for number of channels
    #[arg(long = "as-strategy", short = 's', default_value_t = 1)]
    as_sel_strategy: usize,
    /// The percentage(s) (as a comma-separed list of integers) of congested channels to simulate
    #[arg(long = "congestion-rate", short = 'c', value_delimiter = ',')]
    congestion_rate: Vec<usize>,
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
    let as_selection_strategy = match args.as_sel_strategy {
        0 => AsSelectionStrategy::MaxNodes,
        1 => AsSelectionStrategy::MaxChannels,
        _ => {
            warn!(
                "Invalid AsSelectionStrategy. Defaulting to {:?}",
                AsSelectionStrategy::MaxNodes
            );
            AsSelectionStrategy::MaxNodes
        }
    };
    let results = Arc::new(Mutex::new(Vec::with_capacity(amounts.len())));
    let pairs = simlib::Simulation::draw_n_pairs_for_simulation(&graph, args.num_pairs);
    amounts.par_iter().for_each(|amount| {
        info!("Starting simulation for {amount} sat.");
        let msat = simlib::to_millisatoshi(*amount);
        let mut builder = SimBuilder::new(
            args.run,
            &graph,
            msat,
            args.num_adv_as,
            as_selection_strategy,
        );
        let baseline = builder.simulate(pairs.clone());
        let per_strategy_results = asn_simulation(&builder, baseline, args.congestion_rate.clone());
        let sim_output = SimOutput {
            amt_sat: *amount,
            total_num_payments: args.num_pairs,
            per_strategy_results,
        };
        results.lock().unwrap().push(sim_output);
        info!("Completed simulation for {amount} sat.");
    });
    let sim_report = if let Ok(s) = results.lock() {
        Report {
            run: args.run,
            censor_sim_output: s.clone(),
        }
    } else {
        Report {
            run: args.run,
            ..Default::default()
        }
    };

    sim_report
        .write_to_file(output_dir)
        .expect("Failed to write report to file.");
}

/// Returns the simulation results for each packet drop strategy
fn asn_simulation(
    sim_builder: &SimBuilder,
    baseline_result: simlib::SimResult,
    congestion_rate: Vec<usize>,
) -> Vec<PerStrategyResults> {
    let mut per_strategy_results = vec![];
    let as_ip_map = AsIpMap::new(&sim_builder.graph, false);
    let attack_asns = sim_builder.get_adverserial_asns(&as_ip_map);
    let mut drop_strategies = vec![
        PacketDropStrategy::All,
        PacketDropStrategy::IntraAs,
        PacketDropStrategy::InterAs,
    ];
    for perc in congestion_rate {
        drop_strategies.push(PacketDropStrategy::Congestion {
            congestion_rate: perc,
            graph: sim_builder.graph.clone(),
        });
    }
    for strategy in drop_strategies.iter() {
        let mut attack_results = vec![];
        let intra_as_channel_ratios = if *strategy == PacketDropStrategy::IntraProbability {
            as_ip_map.get_intra_as_channels_ratio(&sim_builder.graph)
        } else {
            HashMap::default()
        };
        for (asn, nodes) in attack_asns.iter() {
            let mut attack_sim = SimBuilder::per_asn_simulation(
                baseline_result.clone(),
                *asn,
                nodes,
                strategy.clone(),
                intra_as_channel_ratios.get(asn),
                &as_ip_map,
            );
            // add the baseline results
            let payments_dist = get_inter_payments_dist(&baseline_result, &as_ip_map);
            let involved_asns = get_num_involved_asns(&baseline_result, &as_ip_map);
            attack_sim.sim_results.insert(
                0,
                SimResult::from_simlib_results(
                    baseline_result.clone(),
                    0,
                    Some(payments_dist),
                    Some(involved_asns),
                ),
            );
            attack_results.push(attack_sim);
        }
        per_strategy_results.push(PerStrategyResults {
            strategy: strategy.clone(),
            attack_results,
        })
    }
    per_strategy_results
}

fn get_inter_payments_dist(sim_result: &simlib::SimResult, as_ip_map: &AsIpMap) -> PaymentsDist {
    let mut inter_failed = 0.0;
    let mut inter_succ = 0.0;

    for payment in sim_result.successful_payments.iter() {
        let src_asn = as_ip_map
            .get_asn_for_node(&payment.source)
            .unwrap_or_default();
        let dst_asn = as_ip_map
            .get_asn_for_node(&payment.dest)
            .unwrap_or_default();
        if src_asn == dst_asn {
            inter_succ += 1.0;
        }
    }
    for payment in sim_result.failed_payments.iter() {
        if let (Some(src_asn), Some(dst_asn)) = (
            as_ip_map.get_asn_for_node(&payment.source),
            as_ip_map.get_asn_for_node(&payment.dest),
        ) {
            if src_asn == dst_asn {
                inter_failed += 1.0;
            }
        }
    }
    PaymentsDist {
        inter_succ: inter_succ / sim_result.successful_payments.len() as f32,
        inter_failed: inter_failed / sim_result.failed_payments.len() as f32,
    }
}

fn get_num_involved_asns(sim_result: &simlib::SimResult, as_ip_map: &AsIpMap) -> InvolvedAsns {
    let mut num_asns = vec![];
    for payment in sim_result.successful_payments.iter() {
        for path in &payment.used_paths {
            let asns = as_ip_map.get_involved_asns(&path.path.get_involved_nodes());
            num_asns.push(asns.len());
        }
    }

    InvolvedAsns { num_asns }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_parser::GraphSource::*;
    use simlib::graph::Graph;
    use std::path::Path;

    #[test]
    fn baseline_to_as_results() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/trivial_connected_lnd.json"),
                Lnd,
            )
            .unwrap(),
            Lnd,
        );
        let amt_msat = 1000;
        let num_adv_as = 1;
        let run = 0;
        let num_pairs = 3;
        let congestion_rate = vec![5, 10];
        let mut sim_builder = SimBuilder::new(
            run,
            &graph,
            amt_msat,
            num_adv_as,
            AsSelectionStrategy::MaxNodes,
        );
        let pairs = simlib::Simulation::draw_n_pairs_for_simulation(&graph, num_pairs);
        let baseline_result = sim_builder.simulate(pairs);
        let actual = asn_simulation(&sim_builder, baseline_result, congestion_rate);
        assert_eq!(actual.len(), 5);
    }

    #[test]
    fn payment_distribution() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let mut sim = SimBuilder::new(
            4711,
            &graph,
            1000,
            0,
            crate::AsSelectionStrategy::MaxChannels,
        );
        let pairs = [
            ("alice".to_string(), "bob".to_string()),
            ("chan".to_string(), "dina".to_string()),
            ("chan".to_string(), "bob".to_string()),
            ("alice".to_string(), "dina".to_string()),
        ];
        let sim_result = sim.simulate(pairs.into_iter());
        let as_ip_map = AsIpMap::new(&graph, false);
        let actual = get_inter_payments_dist(&sim_result, &as_ip_map);
        assert!(actual.inter_succ.is_normal() || actual.inter_failed.is_normal());
    }
}
