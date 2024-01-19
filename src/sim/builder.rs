use crate::{
    net::{AsIpMap, Asn},
    sim::output::*,
};
#[cfg(not(test))]
use log::info;
use simlib::{graph::Graph, PaymentParts, RoutingMetric, Simulation, ID};
#[cfg(test)]
use std::println as info;

use super::output::SimOutput;

pub struct SimBuilder {
    pub(crate) graph: Graph,
    /// Amount to simulate in milli satoshis
    pub(crate) amt_msat: usize,
    /// The number of payments to simulate
    pub(crate) num_payments: usize,
    /// The top-n adversarial ASs
    pub(crate) num_adv_as: usize,
}

impl SimBuilder {
    pub fn new(graph: &Graph, amt_msat: usize, num_payments: usize, num_adv_as: usize) -> Self {
        Self {
            graph: graph.clone(),
            amt_msat,
            num_payments,
            num_adv_as,
        }
    }

    /// Simulate payments with different ASs attacking up to 5 nodes and return a SimOutput
    /// aggregating the outcome
    pub fn simulate(&mut self) -> SimOutput {
        let attack_asns = self.get_adverserial_asns();
        let mut sim_output = SimOutput {
            amt_sat: simlib::to_sat(self.amt_msat),
            attack_sim: Vec::with_capacity(attack_asns.len()),
            ..Default::default()
        };
        let pairs = Simulation::draw_n_pairs_for_simulation(&self.graph, self.num_payments);
        let run = 0;
        let mut baseline_sim = Simulation::new(
            run,
            self.graph.clone(),
            self.amt_msat,
            RoutingMetric::MinFee,
            PaymentParts::Split,
            Some(vec![0]),
            &[],
        );
        let baseline_result = baseline_sim.run(pairs.clone(), None, false);
        sim_output.baseline_sim = SimResult::from_simlib_results(baseline_result, 0);
        for (asn, nodes) in attack_asns.iter() {
            let attack_sim = self.per_asn_simulation(pairs.clone(), *asn, nodes, run);
            sim_output.attack_sim.push(attack_sim);
        }
        sim_output
    }

    fn per_asn_simulation(
        &self,
        pairs: impl Iterator<Item = (ID, ID)> + Clone,
        asn: Asn,
        nodes: &[ID],
        run: u64,
    ) -> AttackSim {
        let max_nodes_under_attack = std::cmp::min(5, nodes.len());
        info!(
            "Simulating up to {} nodes under attack by AS {}.",
            max_nodes_under_attack, asn
        );
        let mut summary = AttackSim {
            asn,
            ..Default::default()
        };
        let mut sim_results = vec![];
        let mut sim_graph = self.graph.clone();
        for (n, node) in nodes.iter().enumerate() {
            if n == max_nodes_under_attack {
                break;
            }
            sim_graph.remove_node(node);
            let mut sim = Simulation::new(
                run,
                sim_graph.clone(),
                self.amt_msat,
                RoutingMetric::MinFee,
                PaymentParts::Split,
                Some(vec![0]),
                &[],
            );
            let sim_result = sim.run(pairs.to_owned(), None, false);
            sim_results.push(SimResult::from_simlib_results(sim_result, n + 1));
        }
        summary.sim_results = sim_results;
        info!("Completed attack by AS {} simulation.", asn);
        summary
    }

    fn get_adverserial_asns(&self) -> Vec<(Asn, Vec<ID>)> {
        let nodes = self.graph.get_nodes();
        let nodes_wo_address = nodes
            .iter()
            .map(|n| n.addresses.len())
            .filter(|n| *n < 1)
            .count() as f32;
        info!(
            "{}% of nodes without a network address",
            (nodes_wo_address / nodes.len() as f32) * 100.0
        );
        let as_ip_map = AsIpMap::new(&nodes);
        let num_adv_as = std::cmp::min(self.num_adv_as, as_ip_map.as_to_nodes.len());
        info!("Simulating top {} ASs as adversaries.", num_adv_as);
        as_ip_map.top_n_asns(num_adv_as, &self.graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_parser::GraphSource::*;
    use simlib::graph::Graph;
    use std::path::Path;

    #[test]
    fn init() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/trivial_connected_lnd.json"),
                Lnd,
            )
            .unwrap(),
            Lnd,
        );
        let amt_msat = 1000;
        let num_pairs = 3;
        let num_adv_as = 1;
        let actual = SimBuilder::new(&graph, amt_msat, num_pairs, num_adv_as);
        let expected = SimBuilder {
            graph: graph.clone(),
            amt_msat: 1000,
            num_payments: 3,
            num_adv_as: 1,
        };
        assert_eq!(actual.graph.node_count(), expected.graph.node_count());
        assert_eq!(actual.num_payments, expected.num_payments);
        assert_eq!(actual.amt_msat, expected.amt_msat);
        assert_eq!(actual.num_adv_as, expected.num_adv_as);
    }

    #[test]
    fn adversarial_asns() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let amt_msat = 1000;
        let num_pairs = 3;
        let num_adv_as = 1;
        let sim_builder = SimBuilder::new(&graph, amt_msat, num_pairs, num_adv_as);
        let actual = sim_builder.get_adverserial_asns();
        let expected = vec![(24940, vec!["bob".to_owned(), "alice".to_owned()])];
        assert_eq!(actual, expected);
    }

    #[test]
    fn simulation() {
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
        let mut builder = SimBuilder::new(&graph, amt_msat, num_pairs, num_adv_as);
        let actual = builder.simulate();
        let expected = SimOutput {
            amt_sat: 1000,
            baseline_sim: SimResult {
                num_nodes: 0,
                num_failed: 0,
                num_successful: 3,
            },
            attack_sim: vec![AttackSim {
                asn: 24940,
                sim_results: vec![SimResult {
                    num_nodes: 1,
                    num_failed: 3,
                    num_successful: 0,
                }],
            }],
        };
        assert_eq!(actual.amt_sat, expected.amt_sat);
        assert_eq!(
            actual.baseline_sim.num_nodes,
            expected.baseline_sim.num_nodes
        );
        assert_eq!(actual.attack_sim.len(), expected.attack_sim.len());
        for i in 0..actual.attack_sim.len() {
            assert_eq!(actual.attack_sim[i].asn, expected.attack_sim[i].asn);
        }
    }
}
