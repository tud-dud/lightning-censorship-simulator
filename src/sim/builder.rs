use crate::{
    net::{AsIpMap, Asn},
    sim::output::*,
    AsSelectionStrategy,
};
#[cfg(not(test))]
use log::info;
use simlib::{graph::Graph, PaymentParts, RoutingMetric, Simulation, ID};
#[cfg(test)]
use std::println as info;

use super::output::SimOutput;

pub struct SimBuilder {
    pub(crate) run: u64,
    pub(crate) graph: Graph,
    /// Amount to simulate in milli satoshis
    pub(crate) amt_msat: usize,
    /// The number of payments to simulate
    pub(crate) num_payments: usize,
    /// The top-n adversarial ASs
    pub(crate) num_adv_as: usize,
    pub(crate) as_selection: AsSelectionStrategy,
}

impl SimBuilder {
    pub fn new(
        run: u64,
        graph: &Graph,
        amt_msat: usize,
        num_payments: usize,
        num_adv_as: usize,
        as_selection: AsSelectionStrategy,
    ) -> Self {
        Self {
            run,
            graph: graph.clone(),
            amt_msat,
            num_payments,
            num_adv_as,
            as_selection,
        }
    }

    /// Simulate payments with different ASs attacking up to 5 nodes and return a SimOutput
    /// aggregating the outcome
    pub fn simulate(&mut self) -> SimOutput {
        let attack_asns = self.get_adverserial_asns();
        let mut sim_output = SimOutput {
            total_num_payments: self.num_payments,
            amt_sat: simlib::to_sat(self.amt_msat),
            attack_results: Vec::with_capacity(attack_asns.len() + 1),
        };
        let pairs = Simulation::draw_n_pairs_for_simulation(&self.graph, self.num_payments);
        let mut baseline_sim = Simulation::new(
            self.run,
            self.graph.clone(),
            self.amt_msat,
            RoutingMetric::MinFee,
            PaymentParts::Split,
            Some(vec![0]),
            &[],
        );
        let baseline_result = baseline_sim.run(pairs.clone(), None, false);
        for (asn, nodes) in attack_asns.iter() {
            let mut attack_sim = self.per_asn_simulation(pairs.clone(), *asn, nodes, self.run);
            attack_sim.sim_results.insert(
                0,
                SimResult::from_simlib_results(baseline_result.clone(), 0),
            );
            sim_output.attack_results.push(attack_sim);
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
        let max_nodes_under_attack = nodes.len();
        info!(
            "Simulating {} nodes under attack by AS {}.",
            max_nodes_under_attack, asn
        );
        let mut summary = AttackSim {
            asn,
            ..Default::default()
        };
        let mut sim_results = vec![];
        let mut sim_graph = self.graph.clone();
        for node in nodes.iter() {
            sim_graph.remove_node(node);
        }
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
        sim_results.push(SimResult::from_simlib_results(sim_result, nodes.len()));
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
        let as_ip_map = AsIpMap::new(&self.graph);
        let num_adv_as = std::cmp::min(self.num_adv_as, as_ip_map.as_to_nodes.len());
        info!(
            "Simulating {} {:?} ASs as adversaries.",
            num_adv_as, self.as_selection
        );
        match self.as_selection {
            AsSelectionStrategy::MaxNodes => as_ip_map.top_n_asns_nodes(num_adv_as, &self.graph),
            AsSelectionStrategy::MaxChannels => {
                as_ip_map.top_n_asns_channels(num_adv_as, &self.graph)
            }
        }
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
        let run = 0;
        let actual = SimBuilder::new(
            run,
            &graph,
            amt_msat,
            num_pairs,
            num_adv_as,
            AsSelectionStrategy::MaxChannels,
        );
        let expected = SimBuilder {
            run,
            graph: graph.clone(),
            amt_msat: 1000,
            num_payments: 3,
            num_adv_as: 1,
            as_selection: AsSelectionStrategy::MaxChannels,
        };
        assert_eq!(actual.graph.node_count(), expected.graph.node_count());
        assert_eq!(actual.num_payments, expected.num_payments);
        assert_eq!(actual.amt_msat, expected.amt_msat);
        assert_eq!(actual.num_adv_as, expected.num_adv_as);
        assert_eq!(actual.as_selection, expected.as_selection);
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
        let run = 0;
        let sim_builder = SimBuilder::new(
            run,
            &graph,
            amt_msat,
            num_pairs,
            num_adv_as,
            AsSelectionStrategy::MaxNodes,
        );
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
        let run = 0;
        let mut builder = SimBuilder::new(
            run,
            &graph,
            amt_msat,
            num_pairs,
            num_adv_as,
            AsSelectionStrategy::MaxNodes,
        );
        let actual = builder.simulate();
        let expected = SimOutput {
            amt_sat: 1000,
            total_num_payments: num_pairs,
            attack_results: vec![AttackSim {
                asn: 24940,
                sim_results: vec![
                    SimResult {
                        num_nodes_under_attack: 0,
                        num_failed: 0,
                        num_successful: 3,
                        payments: vec![],
                    },
                    SimResult {
                        num_nodes_under_attack: 1,
                        num_failed: 3,
                        num_successful: 0,
                        payments: vec![],
                    },
                ],
            }],
        };
        assert_eq!(actual.amt_sat, expected.amt_sat);
        assert_eq!(actual.attack_results.len(), expected.attack_results.len());
        for i in 0..actual.attack_results.len() {
            assert_eq!(actual.attack_results[i].asn, expected.attack_results[i].asn);
        }
    }
}
