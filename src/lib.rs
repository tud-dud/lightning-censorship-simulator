mod net;
mod sim;

pub use sim::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AsSelectionStrategy {
    MaxNodes = 0,
    MaxChannels = 1,
}
