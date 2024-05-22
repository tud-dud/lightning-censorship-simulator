use super::{output::*, SimBuilder};
use crate::{net::Asn, AsIpMap};
use rand::{seq::SliceRandom, thread_rng, Rng};
use simlib::ID;

impl SimBuilder {
    /// Uniformly select a ratio then generate a Boolean outcome for that
    pub(crate) fn apply_prob_drop_strategy(
        sim_result: simlib::SimResult,
        ratios: &Vec<f32>,
        asn_nodes: &[ID],
        asn: Asn,
        as_ip_map: &AsIpMap,
    ) -> (simlib::SimResult, Option<PerSimAccuracy>) {
        let mut updated_results = simlib::SimResult {
            num_failed: sim_result.num_failed,
            num_succesful: 0,
            total_num: sim_result.total_num,
            successful_payments: vec![],
            failed_payments: sim_result.failed_payments,
            ..Default::default()
        };
        let (mut tpos, mut fpos, mut fneg) = (0, 0, 0);
        let mut rng = thread_rng();
        for mut p in sim_result.successful_payments {
            let dest_asn =
                crate::find_key_for_value(&as_ip_map.as_to_nodes, &p.dest).unwrap_or_default();
            if Self::payment_involves_asn(&p, asn_nodes) {
                // only payments affected by the censor
                if let Some(prob) = ratios.choose(&mut rng) {
                    let payment_fate = rng.gen_bool(*prob as f64);
                    if payment_fate {
                        // dropped
                        p.succeeded = false;
                        p.used_paths = vec![];
                        updated_results.num_failed += 1;
                        updated_results.failed_payments.push(p);
                        if dest_asn == asn {
                            tpos += 1;
                        } else {
                            fpos += 1;
                        }
                    } else {
                        // succeeded
                        updated_results.num_succesful += 1;
                        updated_results.successful_payments.push(p);
                        if dest_asn == asn {
                            fneg += 1;
                        }
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
        (updated_results, Some(PerSimAccuracy { tpos, fpos, fneg }))
    }

    /// All packets involving the AS's nodes are dropped
    pub(crate) fn apply_all_dropped_strategy(
        sim_result: simlib::SimResult,
        asn_nodes: &[ID],
    ) -> (simlib::SimResult, Option<PerSimAccuracy>) {
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
        (updated_results, None)
    }

    /// All packets coming from/to asn are dropped
    pub(crate) fn apply_intra_as_drop_strategy(
        sim_result: simlib::SimResult,
        asn: u32,
        as_ip_map: &AsIpMap,
    ) -> (simlib::SimResult, Option<PerSimAccuracy>) {
        let mut updated_results = simlib::SimResult {
            num_failed: sim_result.num_failed,
            num_succesful: 0,
            total_num: sim_result.total_num,
            successful_payments: vec![],
            failed_payments: sim_result.failed_payments,
            ..Default::default()
        };
        for mut p in sim_result.successful_payments {
            let src_asn =
                crate::find_key_for_value(&as_ip_map.as_to_nodes, &p.dest).unwrap_or_default();
            let dest_asn =
                crate::find_key_for_value(&as_ip_map.as_to_nodes, &p.source).unwrap_or_default();
            if src_asn == asn && dest_asn == asn {
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
        (updated_results, None)
    }

    /// All packets leaving asn are dropped
    pub(crate) fn apply_inter_as_drop_strategy(
        sim_result: simlib::SimResult,
        asn: u32,
        as_ip_map: &AsIpMap,
    ) -> (simlib::SimResult, Option<PerSimAccuracy>) {
        let mut updated_results = simlib::SimResult {
            num_failed: sim_result.num_failed,
            num_succesful: 0,
            total_num: sim_result.total_num,
            successful_payments: vec![],
            failed_payments: sim_result.failed_payments,
            ..Default::default()
        };
        let as_nodes = as_ip_map.as_to_nodes.get(&asn).unwrap();
        for mut p in sim_result.successful_payments {
            if Self::payment_involves_asn(&p, as_nodes) {
                let src_asn =
                    crate::find_key_for_value(&as_ip_map.as_to_nodes, &p.dest).unwrap_or_default();
                let dest_asn = crate::find_key_for_value(&as_ip_map.as_to_nodes, &p.source)
                    .unwrap_or_default();
                if src_asn != asn || dest_asn != asn {
                    p.succeeded = false;
                    p.used_paths = vec![];
                    updated_results.num_failed += 1;
                    updated_results.failed_payments.push(p.clone());
                } else {
                    // does not leave the AS so leave as is
                    updated_results.num_succesful += 1;
                    updated_results.successful_payments.push(p);
                }
            } else {
                // does not involve any AS node so leave as is
                updated_results.num_succesful += 1;
                updated_results.successful_payments.push(p);
            }
        }
        (updated_results, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_parser::GraphSource::*;
    use simlib::{graph::Graph, payment::Payment, CandidatePath};
    use std::{collections::VecDeque, path::Path};

    // TODO: Check returned accuracy scores
    #[test]
    fn apply_prob_drop() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let ratios = vec![1.0];
        let asn_nodes = vec!["alice".to_owned()];
        let as_ip_map = AsIpMap::new(&graph, false);
        let asn = 24940;
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
        let (actual_sim_result, _) = SimBuilder::apply_prob_drop_strategy(
            sim_result.clone(),
            &ratios,
            &asn_nodes,
            asn,
            &as_ip_map,
        );
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(
            actual_sim_result.total_num,
            actual_sim_result.num_succesful + actual_sim_result.num_failed
        );
        assert_eq!(
            actual_sim_result.num_succesful,
            actual_sim_result.successful_payments.len()
        );
        assert_eq!(
            actual_sim_result.num_failed,
            actual_sim_result.failed_payments.len()
        );
        assert_eq!(actual_sim_result.num_failed, sim_result.num_failed);

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
        let (actual_sim_result, _) = SimBuilder::apply_prob_drop_strategy(
            sim_result.clone(),
            &ratios,
            &asn_nodes,
            asn,
            &as_ip_map,
        );
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(
            actual_sim_result.total_num,
            actual_sim_result.num_succesful + actual_sim_result.num_failed
        );
        assert!(actual_sim_result.num_failed > sim_result.num_failed);
        assert_eq!(actual_sim_result.num_failed, 3);
        assert_eq!(
            actual_sim_result.num_failed,
            actual_sim_result.failed_payments.len()
        );

        let ratios = vec![0.0]; // no additional failures
        let (actual_sim_result, _) = SimBuilder::apply_prob_drop_strategy(
            sim_result.clone(),
            &ratios,
            &asn_nodes,
            asn,
            &as_ip_map,
        );
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(
            actual_sim_result.total_num,
            actual_sim_result.num_succesful + actual_sim_result.num_failed
        );
        assert_eq!(actual_sim_result.num_failed, sim_result.num_failed);
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
        let (actual_sim_result, _) =
            SimBuilder::apply_all_dropped_strategy(sim_result.clone(), &asn_nodes);
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(actual_sim_result.num_failed, sim_result.num_failed);
        assert_eq!(
            actual_sim_result.total_num,
            actual_sim_result.num_succesful + actual_sim_result.num_failed
        );
        assert_eq!(
            actual_sim_result.num_succesful,
            actual_sim_result.successful_payments.len()
        );
        assert_eq!(
            actual_sim_result.num_failed,
            actual_sim_result.failed_payments.len()
        );
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
        let (actual_sim_result, actual_accuracy) =
            SimBuilder::apply_all_dropped_strategy(sim_result.clone(), &asn_nodes);
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(
            actual_sim_result.total_num,
            actual_sim_result.num_succesful + actual_sim_result.num_failed
        );
        assert!(actual_sim_result.num_failed > sim_result.num_failed);
        assert_eq!(actual_sim_result.num_failed, 3);
        assert_eq!(
            actual_sim_result.num_failed,
            actual_sim_result.failed_payments.len()
        );
        assert!(actual_accuracy.is_none());
    }

    #[test]
    fn apply_intra_as_drop() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let as_ip_map = AsIpMap::new(&graph, false);
        let asn = 797;
        // should pass because dest is not in ASN 797
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
        let mut sim_result = simlib::SimResult {
            num_succesful: 2,
            num_failed: 1,
            total_num: 3,
            successful_payments: vec![successful_payment],
            failed_payments: vec![Payment::new(
                1,
                String::from("chan"),
                String::from("bob"),
                1,
                None,
            )],
            ..Default::default()
        };
        // should fail because both are in ASN 797
        let mut successful_payment =
            Payment::new(0, String::from("dina"), String::from("chan"), 1, None);
        let mut path = simlib::Path::new(String::from("dina"), String::from("chan"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "".to_string()),
        ]);
        successful_payment.used_paths = vec![CandidatePath::new_with_path(path)];
        sim_result.successful_payments.push(successful_payment);
        let (actual_sim_result, _) =
            SimBuilder::apply_intra_as_drop_strategy(sim_result.clone(), asn, &as_ip_map);
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(actual_sim_result.num_succesful, 1);
        assert_eq!(actual_sim_result.num_failed, 2); // the initial one + dina to chan
        assert_eq!(
            actual_sim_result.num_succesful,
            actual_sim_result.successful_payments.len()
        );
        assert_eq!(
            actual_sim_result.num_failed,
            actual_sim_result.failed_payments.len()
        );
        let asn = 24940;
        let (actual_sim_result, _) =
            SimBuilder::apply_intra_as_drop_strategy(sim_result.clone(), asn, &as_ip_map);
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(actual_sim_result.num_succesful, 2);
        assert_eq!(actual_sim_result.num_failed, 1); // nothing changes
    }

    #[test]
    fn apply_inter_as_drop() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let as_ip_map = AsIpMap::new(&graph, false);
        let asn = 797;
        // should fail as the source is in asn 797
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
        let mut sim_result = simlib::SimResult {
            num_succesful: 3,
            num_failed: 1,
            total_num: 4,
            successful_payments: vec![successful_payment],
            failed_payments: vec![Payment::new(
                1,
                String::from("chan"),
                String::from("bob"),
                1,
                None,
            )],
            ..Default::default()
        };
        // should pass since it doesnt leave the AS
        let mut successful_payment =
            Payment::new(0, String::from("dina"), String::from("chan"), 1, None);
        let mut path = simlib::Path::new(String::from("dina"), String::from("chan"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "".to_string()),
        ]);
        successful_payment.used_paths = vec![CandidatePath::new_with_path(path)];
        sim_result.successful_payments.push(successful_payment);
        // should pass since it doesnt involve the AS
        let mut successful_payment =
            Payment::new(0, String::from("bob"), String::from("alice"), 1, None);
        let mut path = simlib::Path::new(String::from("bob"), String::from("alice"));
        path.hops = VecDeque::from([
            ("bob".to_string(), 0, 0, "".to_string()),
            ("alice".to_string(), 0, 0, "".to_string()),
        ]);
        successful_payment.used_paths = vec![CandidatePath::new_with_path(path)];
        sim_result.successful_payments.push(successful_payment);
        let (actual_sim_result, _) =
            SimBuilder::apply_inter_as_drop_strategy(sim_result.clone(), asn, &as_ip_map);
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(actual_sim_result.num_succesful, 2); // dina to bob, bob to alice
        assert_eq!(actual_sim_result.num_failed, 2);
        assert_eq!(
            actual_sim_result.total_num,
            actual_sim_result.num_succesful + actual_sim_result.num_failed
        );
        assert_eq!(
            actual_sim_result.num_succesful,
            actual_sim_result.successful_payments.len()
        );
        assert_eq!(
            actual_sim_result.num_failed,
            actual_sim_result.failed_payments.len()
        );
        let asn = 24940;
        let (actual_sim_result, _) =
            SimBuilder::apply_inter_as_drop_strategy(sim_result.clone(), asn, &as_ip_map);
        assert_eq!(actual_sim_result.total_num, sim_result.total_num);
        assert_eq!(actual_sim_result.num_succesful, 2);
        assert_eq!(actual_sim_result.num_failed, 2); // dina to bob
    }
}
