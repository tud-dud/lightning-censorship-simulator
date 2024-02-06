use crate::{
    net::{AsIpMap, Asn},
    AsSelectionStrategy, PacketDropStrategy,
};
#[cfg(not(test))]
use log::info;
use simlib::{graph::Graph, payment::Payment, ID};
#[cfg(test)]
use std::println as info;

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
    pub(crate) drop_strategy: PacketDropStrategy,
}

impl SimBuilder {
    pub fn new(
        run: u64,
        graph: &Graph,
        amt_msat: usize,
        num_payments: usize,
        num_adv_as: usize,
        as_selection: AsSelectionStrategy,
        drop_strategy: PacketDropStrategy,
    ) -> Self {
        Self {
            run,
            graph: graph.clone(),
            amt_msat,
            num_payments,
            num_adv_as,
            as_selection,
            drop_strategy,
        }
    }

    pub(super) fn get_adverserial_asns(&self, as_ip_map: &AsIpMap) -> Vec<(Asn, Vec<ID>)> {
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
    pub(super) fn payment_involves_asn(payment: &Payment, asn_nodes: &[ID]) -> bool {
        for path in payment.used_paths.iter() {
            let involved_nodes = path.path.get_involved_nodes();
            for hop in involved_nodes {
                if asn_nodes.contains(&hop) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_parser::GraphSource::*;
    use simlib::{graph::Graph, CandidatePath};
    use std::{collections::VecDeque, path::Path};

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
            PacketDropStrategy::All,
        );
        let expected = SimBuilder {
            run,
            graph: graph.clone(),
            amt_msat: 1000,
            num_payments: 3,
            num_adv_as: 1,
            as_selection: AsSelectionStrategy::MaxChannels,
            drop_strategy: PacketDropStrategy::All,
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
            PacketDropStrategy::All,
        );
        let actual = sim_builder.get_adverserial_asns(&AsIpMap::new(&graph, true));
        let expected = vec![(24940, vec!["bob".to_owned(), "alice".to_owned()])];
        assert_eq!(actual, expected);
    }

    #[test]
    fn involved_adversaries() {
        let asn_nodes = vec!["alice".to_owned()];
        let mut path = simlib::Path::new(String::from("dina"), String::from("bob"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "c".to_string()),
            ("bob".to_string(), 0, 0, "".to_string()),
        ]);
        let mut payment = Payment::new(0, String::from("dina"), String::from("bob"), 1, None);
        payment.used_paths = vec![CandidatePath::new_with_path(path)];
        let actual = SimBuilder::payment_involves_asn(&payment, &asn_nodes);
        assert!(!actual);
        let mut path = simlib::Path::new(String::from("dina"), String::from("bob"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "c".to_string()),
            ("alice".to_string(), 0, 0, "c".to_string()),
            ("bob".to_string(), 0, 0, "".to_string()),
        ]);
        payment.used_paths.push(CandidatePath::new_with_path(path));
        let actual = SimBuilder::payment_involves_asn(&payment, &asn_nodes);
        assert!(actual);
        let mut path = simlib::Path::new(String::from("dina"), String::from("alice"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("alice".to_string(), 0, 0, "c".to_string()),
        ]);
        payment.used_paths = vec![CandidatePath::new_with_path(path)];
        let actual = SimBuilder::payment_involves_asn(&payment, &asn_nodes);
        assert!(actual);
    }
}
