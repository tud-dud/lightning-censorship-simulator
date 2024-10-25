mod net;
mod sim;

pub use net::*;
use serde::Serialize;
pub use sim::*;
use simlib::graph::Graph;
use std::{collections::HashMap, fmt::Debug};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AsSelectionStrategy {
    MaxNodes = 0,
    MaxChannels = 1,
}

/// An AS with either drop all packets or drop a packet based on the probabilty that it remains
/// within the AS
#[derive(Clone, Default, Serialize)]
pub enum PacketDropStrategy {
    #[default]
    All,
    IntraProbability,
    /// Drop payments from/to nodes in our AS, i.e., the simulator will fail all payments if the
    /// src or dst belong to the attacking AS. Works because we are able to ID the sender/receiver
    /// at each hop
    IntraAs,
    /// Drop payments from/to nodes outside our AS, i.e., the simulator will fail all payments if the
    /// src or dst do not belong to the attacking AS.
    InterAs,
    /// x% of the ASs highest-capacity channels are congested and we thus cannot use them to route
    /// payments
    Congestion {
        congestion_rate: usize,
        #[serde(skip)]
        graph: Graph,
    },
}

impl Debug for PacketDropStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PacketDropStrategy::All => write!(f, "All"),
            PacketDropStrategy::IntraAs => write!(f, "IntraAs"),
            PacketDropStrategy::InterAs => write!(f, "InterAs"),
            PacketDropStrategy::Congestion {
                congestion_rate, ..
            } => write!(f, "{congestion_rate}% Congestion"),
            PacketDropStrategy::IntraProbability => write!(f, "IntraProbability"),
        }
    }
}

impl PartialEq for PacketDropStrategy {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

pub(crate) static TOR_ASN: u32 = 0;

pub(crate) fn find_key_for_value(map: &HashMap<u32, Vec<String>>, value: &String) -> Option<u32> {
    map.iter().find_map(|(key, val)| {
        if val.contains(value) {
            Some(*key)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::path::Path;

    #[test]
    fn pds_equality() {
        assert_ne!(
            PacketDropStrategy::All,
            PacketDropStrategy::IntraProbability
        );
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/trivial_connected_lnd.json"),
                network_parser::GraphSource::Lnd,
            )
            .unwrap(),
            network_parser::GraphSource::Lnd,
        );
        let mut graph2 = graph.clone();
        graph2.set_edges(HashMap::default());
        assert_eq!(
            PacketDropStrategy::Congestion {
                congestion_rate: 5,
                graph: graph.clone()
            },
            PacketDropStrategy::Congestion {
                congestion_rate: 5,
                graph: graph2.clone()
            },
        );
        // TODO: Fix
        //assert_ne!(PacketDropStrategy::Congestion(10, graph.clone()), PacketDropStrategy::Congestion(5, graph));
    }
}
