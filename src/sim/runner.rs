use super::{output::*, SimBuilder};
use crate::{net::Asn, AsIpMap, PacketDropStrategy};
#[cfg(not(test))]
use log::info;
use simlib::{PaymentParts, RoutingMetric, Simulation, ID};
#[cfg(test)]
use std::println as info;

impl SimBuilder {
    /// Simulate payments with different ASs attacking up to 5 nodes and return a SimOutput
    /// aggregating the outcome
    pub fn simulate(&mut self, pairs: impl Iterator<Item = (ID, ID)> + Clone) -> simlib::SimResult {
        let mut baseline_sim = Simulation::new(
            self.run,
            self.graph.clone(),
            self.amt_msat,
            RoutingMetric::MinFee,
            PaymentParts::Split,
            Some(vec![0]),
            &[],
        );
        baseline_sim.run(pairs.clone(), None, false)
    }

    pub fn per_asn_simulation(
        baseline_result: simlib::SimResult,
        asn: Asn,
        nodes: &[ID],
        strategy: PacketDropStrategy,
        ratios: Option<&Vec<f32>>,
        as_ip_map: &AsIpMap,
    ) -> AttackSim {
        let max_nodes_under_attack = nodes.len();
        info!(
            "Simulating {} nodes under attack by AS {}.",
            max_nodes_under_attack, asn
        );
        let mut summary = AttackSim {
            asn: asn.to_string(),
            ..Default::default()
        };
        let mut sim_results = vec![];
        let ((updated_results, per_sim_accuracy), num_nodes) = match strategy {
            PacketDropStrategy::IntraProbability => {
                if let Some(ratios) = ratios {
                    (
                        Self::apply_prob_drop_strategy(
                            baseline_result,
                            ratios,
                            nodes,
                            asn,
                            as_ip_map,
                        ),
                        usize::MAX,
                    )
                } else {
                    ((baseline_result, None), nodes.len())
                }
            }
            PacketDropStrategy::All => (
                Self::apply_all_dropped_strategy(baseline_result, nodes),
                nodes.len(),
            ),
            PacketDropStrategy::IntraAs => (
                Self::apply_intra_as_drop_strategy(baseline_result, asn, as_ip_map),
                usize::MAX,
            ),
            PacketDropStrategy::InterAs => (
                Self::apply_inter_as_drop_strategy(baseline_result, asn, as_ip_map),
                usize::MAX,
            ),
        };
        sim_results.push(SimResult::from_simlib_results(updated_results, num_nodes));
        summary.sim_results = sim_results;
        summary.per_sim_accuracy = per_sim_accuracy;
        info!(
            "Completed simulation of {:?} attack by AS {}.",
            strategy, asn
        );
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsSelectionStrategy;
    use network_parser::GraphSource::*;
    use simlib::graph::Graph;
    use std::path::Path;

    #[test]
    fn baseline_simulation() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/trivial_connected_lnd.json"),
                Lnd,
            )
            .unwrap(),
            Lnd,
        );
        let amt_msat = 1000000;
        let num_pairs = 3;
        let num_adv_as = 1;
        let run = 0;
        let mut builder = SimBuilder::new(
            run,
            &graph,
            amt_msat,
            num_adv_as,
            AsSelectionStrategy::MaxNodes,
        );
        let pairs = simlib::Simulation::draw_n_pairs_for_simulation(&graph, num_pairs);
        let actual = builder.simulate(pairs);
        assert_eq!(actual.run, run);
        assert_eq!(actual.total_num, num_pairs);
        assert_eq!(actual.num_failed + actual.num_succesful, num_pairs);
    }
}
