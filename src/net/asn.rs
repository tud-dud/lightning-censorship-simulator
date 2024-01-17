use super::DbReader;

use simlib::{Node, ID};
use std::{collections::HashMap, str::FromStr};

#[cfg(not(test))]
use log::{info, warn};
#[cfg(test)]
use std::{println as info, println as warn};

pub(crate) type Asn = u32;

pub(crate) struct AsIpMap {
    pub(crate) as_to_nodes: HashMap<Asn, Vec<ID>>,
}

impl AsIpMap {
    pub(crate) fn new(nodes: &[Node]) -> Self {
        let db_reader = DbReader::new();
        let mut as_to_nodes = HashMap::default();
        for node in nodes {
            for addr in &node.addresses {
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
            }
        }
        info!("Found {} ASNs in input graph.", as_to_nodes.len());
        Self { as_to_nodes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_parser::GraphSource::Lnd;
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
}
