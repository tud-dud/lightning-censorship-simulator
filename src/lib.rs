mod net;
mod sim;

pub use net::*;
use serde::Serialize;
pub use sim::*;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AsSelectionStrategy {
    MaxNodes = 0,
    MaxChannels = 1,
}

/// An AS with either drop all packets or drop a packet based on the probabilty that it remains
/// within the AS
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize)]
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
mod tests {}
