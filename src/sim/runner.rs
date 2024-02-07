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
        let mut sim = Simulation::new(
            run,
            self.graph.clone(),
            self.amt_msat,
            RoutingMetric::MinFee,
            PaymentParts::Split,
            Some(vec![0]),
            &[],
        );
        let sim_result = sim.run(pairs.to_owned(), None, false);
        let (updated_results, num_nodes) =
            if self.drop_strategy == PacketDropStrategy::IntraProbability {
                if let Some(ratios) = ratios {
                    (
                        Self::apply_prob_drop_strategy(sim_result, ratios, nodes),
                        usize::MAX,
                    )
                } else {
                    (sim_result, nodes.len())
                }
            } else {
                (
                    Self::apply_all_dropped_strategy(sim_result, nodes),
                    nodes.len(),
                )
            };
        sim_results.push(SimResult::from_simlib_results(updated_results, num_nodes));
        summary.sim_results = sim_results;
        info!(
            "Completed simulation of {:?} attack by AS {}.",
            self.drop_strategy, asn
        );
        summary
    }

    /// Uniformly select a ratio then generate a Boolean outcome for that
    fn apply_prob_drop_strategy(
        sim_result: simlib::SimResult,
        ratios: &Vec<f32>,
        asn_nodes: &[ID],
    ) -> simlib::SimResult {
        let mut updated_results = simlib::SimResult {
            num_failed: sim_result.num_failed,
            num_succesful: 0,
            total_num: sim_result.total_num,
            successful_payments: vec![],
            failed_payments: sim_result.failed_payments,
            ..Default::default()
        };
        let mut rng = thread_rng();
        for mut p in sim_result.successful_payments {
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
        }
        updated_results
    }

    /// All packets involving the AS's nodes are dropped
    fn apply_all_dropped_strategy(
        sim_result: simlib::SimResult,
        asn_nodes: &[ID],
    ) -> simlib::SimResult {
        let mut updated_results = simlib::SimResult {
            num_failed: sim_result.num_failed,
            num_succesful: 0,
            total_num: sim_result.total_num,
            successful_payments: vec![],
            failed_payments: sim_result.failed_payments,
            ..Default::default()
        };
        for mut p in sim_result.successful_payments {
            if Self::payment_involves_asn(&p, asn_nodes) {
                // dropped
                p.succeeded = false;
                p.used_paths = vec![];
                updated_results.num_failed += 1;
                updated_results.failed_payments.push(p);
            } else {
                // does not involve any AS node so leave as is
                updated_results.num_succesful += 1;
                updated_results.successful_payments.push(p);
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
    use simlib::{graph::Graph, payment::Payment, CandidatePath};
    use std::{collections::VecDeque, path::Path};

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

    #[test]
    fn apply_prob_drop() {
        let ratios = vec![1.0];
        let asn_nodes = vec!["alice".to_owned()];
        let mut successful_payment =
            Payment::new(0, String::from("dina"), String::from("bob"), 1, None);
        let mut path = simlib::Path::new(String::from("dina"), String::from("bob"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "c".to_string()),
            ("bob".to_string(), 0, 0, "".to_string()),
        ]);
        successful_payment.succeeded = true;
        successful_payment.used_paths = vec![CandidatePath::new_with_path(path)];
        let sim_result = simlib::SimResult {
            num_succesful: 2,
            num_failed: 1,
            total_num: 3,
            successful_payments: vec![successful_payment.clone(), successful_payment],
            failed_payments: vec![Payment::new(
                1,
                String::from("chan"),
                String::from("bob"),
                1,
                None,
            )],
            ..Default::default()
        };
        let actual = SimBuilder::apply_prob_drop_strategy(sim_result.clone(), &ratios, &asn_nodes);
        assert_eq!(actual.total_num, sim_result.total_num);
        assert_eq!(actual.total_num, actual.num_succesful + actual.num_failed);
        assert_eq!(actual.num_succesful, actual.successful_payments.len());
        assert_eq!(actual.num_failed, actual.failed_payments.len());
        assert_eq!(actual.num_failed, sim_result.num_failed);

        let mut successful_payment =
            Payment::new(0, String::from("dina"), String::from("alice"), 1, None);
        successful_payment.succeeded = true;
        let mut path = simlib::Path::new(String::from("dina"), String::from("alice"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "c".to_string()),
            ("alice".to_string(), 0, 0, "".to_string()),
        ]);
        successful_payment.used_paths = vec![CandidatePath::new_with_path(path)];
        let sim_result = simlib::SimResult {
            num_succesful: 2,
            num_failed: 1,
            total_num: 3,
            successful_payments: vec![successful_payment.clone(), successful_payment],
            failed_payments: vec![Payment::new(
                1,
                String::from("dina"),
                String::from("bob"),
                1,
                None,
            )],
            ..Default::default()
        };
        let actual = SimBuilder::apply_prob_drop_strategy(sim_result.clone(), &ratios, &asn_nodes);
        assert_eq!(actual.total_num, sim_result.total_num);
        assert_eq!(actual.total_num, actual.num_succesful + actual.num_failed);
        assert!(actual.num_failed > sim_result.num_failed);
        assert_eq!(actual.num_failed, 3);
        assert_eq!(actual.num_failed, actual.failed_payments.len());

        let ratios = vec![0.0]; // no additional failures
        let actual = SimBuilder::apply_prob_drop_strategy(sim_result.clone(), &ratios, &asn_nodes);
        assert_eq!(actual.total_num, sim_result.total_num);
        assert_eq!(actual.total_num, actual.num_succesful + actual.num_failed);
        assert_eq!(actual.num_failed, sim_result.num_failed);
    }

    #[test]
    fn apply_all_drop() {
        let asn_nodes = vec!["alice".to_owned()];
        let mut successful_payment =
            Payment::new(0, String::from("dina"), String::from("bob"), 1, None);
        let mut path = simlib::Path::new(String::from("dina"), String::from("bob"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "c".to_string()),
            ("bob".to_string(), 0, 0, "".to_string()),
        ]);
        successful_payment.succeeded = true;
        successful_payment.used_paths = vec![CandidatePath::new_with_path(path)];
        let sim_result = simlib::SimResult {
            num_succesful: 2,
            num_failed: 1,
            total_num: 3,
            successful_payments: vec![successful_payment.clone(), successful_payment],
            failed_payments: vec![Payment::new(
                1,
                String::from("chan"),
                String::from("bob"),
                1,
                None,
            )],
            ..Default::default()
        };
        let actual = SimBuilder::apply_all_dropped_strategy(sim_result.clone(), &asn_nodes);
        assert_eq!(actual.total_num, sim_result.total_num);
        assert_eq!(actual.num_failed, sim_result.num_failed);
        assert_eq!(actual.total_num, actual.num_succesful + actual.num_failed);
        assert_eq!(actual.num_succesful, actual.successful_payments.len());
        assert_eq!(actual.num_failed, actual.failed_payments.len());
        let mut successful_payment =
            Payment::new(0, String::from("dina"), String::from("alice"), 1, None);
        successful_payment.succeeded = true;
        let mut path = simlib::Path::new(String::from("dina"), String::from("alice"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "c".to_string()),
            ("alice".to_string(), 0, 0, "".to_string()),
        ]);
        successful_payment.used_paths = vec![CandidatePath::new_with_path(path)];
        let sim_result = simlib::SimResult {
            num_succesful: 2,
            num_failed: 1,
            total_num: 3,
            successful_payments: vec![successful_payment.clone(), successful_payment],
            failed_payments: vec![Payment::new(
                1,
                String::from("dina"),
                String::from("bob"),
                1,
                None,
            )],
            ..Default::default()
        };
        let actual = SimBuilder::apply_all_dropped_strategy(sim_result.clone(), &asn_nodes);
        assert_eq!(actual.total_num, sim_result.total_num);
        assert_eq!(actual.total_num, actual.num_succesful + actual.num_failed);
        assert!(actual.num_failed > sim_result.num_failed);
        assert_eq!(actual.num_failed, 3);
        assert_eq!(actual.num_failed, actual.failed_payments.len());
    }
}
