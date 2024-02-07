mod net;
mod sim;

pub use net::*;
use serde::Serialize;
pub use sim::*;

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
}

pub(crate) static TOR_ASN: u32 = 0;
