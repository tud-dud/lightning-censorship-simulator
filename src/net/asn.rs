use super::{Asn, DbReader};

use simlib::{graph::Graph, Node, ID};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    str::FromStr,
};

#[cfg(not(test))]
use log::{info, trace, warn};
#[cfg(test)]
use std::{println as info, println as warn, println as trace};

pub(crate) struct AsIpMap {
    pub(crate) as_to_nodes: HashMap<Asn, Vec<ID>>,
}

impl AsIpMap {
    pub(crate) fn new(graph: &Graph) -> Self {
        let db_reader = DbReader::new();
        let mut as_to_nodes = HashMap::default();
        let nodes = graph.get_nodes();
        for node in nodes {
            if let Some(asn) = Self::lookup_asn_for_node(&db_reader, &node) {
                as_to_nodes
                    .entry(asn)
                    .and_modify(|m: &mut Vec<ID>| m.push(node.id.to_owned()))
                    .or_insert(vec![node.id.to_owned()]);
            }
        }
        info!(
            "Found a total of {} ASNs in input graph.",
            as_to_nodes.len()
        );
        Self {
            as_to_nodes,
        }
    }

    /// Returns an ordered list of the n most-represented ASNs w.r.t the number of nodes.
    /// The list of nodes is sorted in descending order of number of channels
    pub(crate) fn top_n_asns_nodes(&self, n: usize, graph: &Graph) -> Vec<(Asn, Vec<ID>)> {
        let mut heap = BinaryHeap::with_capacity(n + 1);
        for (asn, mut nodes) in self.as_to_nodes.clone().into_iter() {
            // sort in descending order
            nodes.sort_by(|a, b| {
                graph
                    .get_edges_for_node(b)
                    .unwrap_or_default()
                    .len()
                    .cmp(&graph.get_edges_for_node(a).unwrap_or_default().len())
            });
            heap.push(Reverse((nodes.len(), asn, nodes.clone())));
            if heap.len() > n {
                heap.pop();
            }
        }
        heap.into_sorted_vec()
            .into_iter()
            .map(|r| (r.0 .1, r.0 .2))
            .collect()
    }

    /// Returns an ordered list of the n most-represented ASNs w.r.t the number of channels.
    /// The list of nodes is sorted in descending order of number of channels
    pub(crate) fn top_n_asns_channels(&self, n: usize, graph: &Graph) -> Vec<(Asn, Vec<ID>)> {
        let mut heap = BinaryHeap::with_capacity(n + 1);
        for (asn, mut nodes) in self.as_to_nodes.clone().into_iter() {
            let sum_channels: usize = nodes.iter().map(|n| graph.get_edges_for_node(n).unwrap_or_default().len()).sum();
            // sort in descending order of number of channels
            nodes.sort_by(|a, b| {
                graph
                    .get_edges_for_node(b)
                    .unwrap_or_default()
                    .len()
                    .cmp(&graph.get_edges_for_node(a).unwrap_or_default().len())
            });
            heap.push(Reverse((sum_channels, asn, nodes.clone())));
            if heap.len() > n {
                heap.pop();
            }
        }
        heap.into_sorted_vec()
            .into_iter()
            .map(|r| (r.0 .1, r.0 .2))
            .collect()
    }

    fn lookup_asn_for_node(db_reader: &DbReader, node: &Node) -> Option<Asn> {
        for addr in &node.addresses {
            if !addr.addr.contains("onion") {
                if let Ok(ip) = FromStr::from_str(&addr.addr) {
                    if let Some(asn) = db_reader.lookup_asn(ip) {
                        return Some(asn);
                    } else {
                        warn!("No ASN entry found for {} in database.", ip);
                    }
                } else {
                    warn!("Unable to convert {:?} to IpAddr.", addr.addr);
                }
            } else {
                trace!("Skipping onion address.");
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use network_parser::{Address, GraphSource::*};
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
        let as_ip_map = AsIpMap::new(&graph);
        let actual = as_ip_map.as_to_nodes;
        let expected = HashMap::from([
            (797, vec!["036".to_owned()]),
            (24940, vec!["025".to_owned(), "034".to_owned()]),
        ]);
        assert_eq!(actual.len(), expected.len());
        for a in actual {
            let expected_nodes = expected.get(&a.0).unwrap();
            assert_eq!(expected_nodes.len(), a.1.len());
        }
    }

    #[test]
    fn asn_lookup() {
        let db_reader = DbReader::new();
        let node = Node::default();
        let actual = AsIpMap::lookup_asn_for_node(&db_reader, &node);
        let expected = None;
        assert_eq!(expected, actual);
        let node = Node {
            addresses: vec![
                Address {
                    network: "tcp".to_string(),
                    addr: "archiveiya74codqgiixo33q62qlrqtkgmcitqx5u2oeqnmn5bpcbiyd.onion"
                        .to_string(),
                },
                Address {
                    network: "tcp".to_string(),
                    addr: "8.8.8.8".to_string(),
                },
            ],
            ..Default::default()
        };
        let actual = AsIpMap::lookup_asn_for_node(&db_reader, &node);
        let expected = Some(15169);
        assert_eq!(expected, actual);
    }
    #[test]
    fn top_k_asns_nodes() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let n = 2;
        let as_ip_map = AsIpMap::new(&graph);
        let actual = as_ip_map.top_n_asns_nodes(n, &graph);
        let expected = vec![
            (24940, vec!["bob".to_owned(), "alice".to_owned()]),
            (797, vec!["chan".to_owned(), "dina".to_owned()]),
        ];
        assert_eq!(actual, expected);
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/trivial_connected_lnd.json"),
                Lnd,
            )
            .unwrap(),
            Lnd,
        );
        let n = 1;
        let as_ip_map = AsIpMap::new(&graph);
        let actual = as_ip_map.top_n_asns_nodes(n, &graph);
        let expected = vec![(24940, vec!["025".to_owned(), "034".to_owned()])];
        assert_eq!(actual.len(), expected.len());
        assert_eq!(actual[0].0, expected[0].0);
        for a in actual[0].1.iter() {
            assert!(expected[0].1.contains(&a));
        }
    }

    #[test]
    fn top_k_asns_channels() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let n = 2;
        let as_ip_map = AsIpMap::new(&graph);
        let actual = as_ip_map.top_n_asns_channels(n, &graph);
        let expected = vec![
            (24940, vec!["bob".to_owned(), "alice".to_owned()]),
            (797, vec!["chan".to_owned(), "dina".to_owned()]),
        ];
        assert_eq!(actual, expected);
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/trivial_connected_lnd.json"),
                Lnd,
            )
            .unwrap(),
            Lnd,
        );
        let n = 1;
        let as_ip_map = AsIpMap::new(&graph);
        let actual = as_ip_map.top_n_asns_channels(n, &graph);
        let expected = vec![(24940, vec!["025".to_owned(), "034".to_owned()])];
        assert_eq!(actual.len(), expected.len());
        assert_eq!(actual[0].0, expected[0].0);
        for a in actual[0].1.iter() {
            assert!(expected[0].1.contains(&a));
        }
    }
}
