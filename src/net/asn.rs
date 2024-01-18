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
    pub(crate) fn new(nodes: &[Node]) -> Self {
        let db_reader = DbReader::new();
        let mut as_to_nodes = HashMap::default();
        for node in nodes {
            for addr in &node.addresses {
                if !addr.addr.contains("onion") {
                    if let Ok(ip) = FromStr::from_str(&addr.addr) {
                        if let Some(asn) = db_reader.lookup_asn(ip) {
                            as_to_nodes
                                .entry(asn)
                                .and_modify(|m: &mut Vec<ID>| m.push(node.id.to_owned()))
                                .or_insert(vec![node.id.to_owned()]);
                            break;
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
        }
        info!(
            "Found a total of {} ASNs in input graph.",
            as_to_nodes.len()
        );
        Self { as_to_nodes }
    }

    /// Returns an ordered list of the n most-represented ASNs.
    /// The list of nodes is sorted in descending order of number of channels
    pub(crate) fn top_n_asns(&self, n: usize, graph: &Graph) -> Vec<(Asn, Vec<ID>)> {
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
            heap.push(Reverse((asn, nodes.clone())));
            if heap.len() > n {
                heap.pop();
            }
        }
        heap.into_sorted_vec().into_iter().map(|r| r.0).collect()
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
        let as_ip_map = AsIpMap::new(&graph.get_nodes());
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
    fn top_k_asns() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let n = 2;
        let as_ip_map = AsIpMap::new(&graph.get_nodes());
        let actual = as_ip_map.top_n_asns(n, &graph);
        let expected = vec![
            (24940, vec!["bob".to_owned(), "alice".to_owned()]),
            (797, vec!["chan".to_owned(), "dina".to_owned()]),
        ];
        assert_eq!(actual, expected);
    }
}
