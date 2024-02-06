use super::{output::*, SimBuilder};
use crate::{
    net::{AsIpMap, Asn},
    PacketDropStrategy,
};
#[cfg(not(test))]
use log::info;
use rand::{seq::SliceRandom, thread_rng, Rng};
use simlib::{PaymentParts, RoutingMetric, Simulation, ID};
#[cfg(test)]
use std::println as info;

impl SimBuilder {
    /// Simulate payments with different ASs attacking up to 5 nodes and return a SimOutput
    /// aggregating the outcome
    pub fn simulate(&mut self, pairs: impl Iterator<Item = (ID, ID)> + Clone) -> SimOutput {
        let as_ip_map = AsIpMap::new(&self.graph, false);
        let attack_asns = self.get_adverserial_asns(&as_ip_map);
        let mut sim_output = SimOutput {
            total_num_payments: self.num_payments,
            amt_sat: simlib::to_sat(self.amt_msat),
            attack_results: Vec::with_capacity(attack_asns.len() + 1),
        };
        let mut baseline_sim = Simulation::new(
            self.run,
            self.graph.clone(),
            self.amt_msat,
            RoutingMetric::MinFee,
            PaymentParts::Split,
            Some(vec![0]),
            &[],
        );
        let intra_as_channel_ratios = as_ip_map.get_intra_as_channels_ratio(&self.graph);
        let baseline_result = baseline_sim.run(pairs.clone(), None, false);
        for (asn, nodes) in attack_asns.iter() {
            let mut attack_sim = self.per_asn_simulation(
                pairs.clone(),
                *asn,
                nodes,
                self.run,
                intra_as_channel_ratios.get(asn),
            );
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
        ratios: Option<&Vec<f32>>,
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
        if self.drop_strategy == PacketDropStrategy::All {
            for node in nodes.iter() {
                sim_graph.remove_node(node);
            }
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
        if self.drop_strategy == PacketDropStrategy::IntraProbability {
            let (updated_results, num_nodes) = if let Some(ratios) = ratios {
                (
                    Self::apply_drop_strategy(sim_result, ratios, nodes),
                    usize::MAX,
                )
            } else {
                (sim_result, nodes.len())
            };
            sim_results.push(SimResult::from_simlib_results(updated_results, num_nodes));
        } else {
            // TODO: Should we hand this case differently? thinking we need to fail the
            // payments after execution
            sim_results.push(SimResult::from_simlib_results(sim_result, nodes.len()));
        }
        summary.sim_results = sim_results;
        info!("Completed attack by AS {} simulation.", asn);
        summary
    }

    /// Uniformly select a ratio then generate a Boolean outcome for that
    fn apply_drop_strategy(
        sim_result: simlib::SimResult,
        ratios: &Vec<f32>,
        asn_nodes: &[ID],
    ) -> simlib::SimResult {
        let mut updated_results = simlib::SimResult {
            num_failed: 0,
            num_succesful: 0,
            successful_payments: vec![],
            failed_payments: vec![],
            ..Default::default()
        };
        let mut all_payments = sim_result.successful_payments;
        all_payments.extend(sim_result.failed_payments);
        let mut rng = thread_rng();
        for mut p in all_payments {
            if p.succeeded {
                if Self::payment_involves_asn(&p, asn_nodes) {
                    if let Some(prob) = ratios.choose(&mut rng) {
                        let payment_fate = rng.gen_bool(*prob as f64);
                        if payment_fate {
                            // dropped
                            p.succeeded = false;
                            p.used_paths = vec![];
                            updated_results.num_failed += 1;
                            updated_results.failed_payments.push(p);
                        } else {
                            // succeeded
                            updated_results.num_succesful += 1;
                            updated_results.successful_payments.push(p);
                        }
                    } else {
                        // weird case but lets leave the payment as is
                        updated_results.num_succesful += 1;
                        updated_results.successful_payments.push(p);
                    }
                } else {
                    // no choice to make here
                    updated_results.num_succesful += 1;
                    updated_results.successful_payments.push(p);
                }
            } else {
                // it failed anyway so doesn't matter
                updated_results.num_failed += 1;
                p.succeeded = false;
                p.used_paths = vec![];
                updated_results.failed_payments.push(p);
            }
        }
        updated_results
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
            PacketDropStrategy::All,
        );
        let pairs = simlib::Simulation::draw_n_pairs_for_simulation(&graph, num_pairs);
        let actual = builder.simulate(pairs);
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
