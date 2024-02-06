mod net;
mod sim;

pub use net::*;
pub use sim::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AsSelectionStrategy {
    MaxNodes = 0,
    MaxChannels = 1,
}

/// An AS with either drop all packets or drop a packet based on the probabilty that it remains
/// within the AS
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PacketDropStrategy {
    All = 0,
    IntraProbability = 1,
}
