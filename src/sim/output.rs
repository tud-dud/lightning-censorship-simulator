use crate::net::Asn;

#[derive(Debug, Default, PartialEq)]
pub struct SimOutput {
    pub amt_sat: usize,
    /// Sim results when no nodes are under attack
    pub baseline_sim: SimResult,
    pub attack_sim: Vec<AttackSim>,
}

#[derive(Debug, Default, PartialEq)]
pub struct AttackSim {
    pub asn: Asn,
    pub sim_results: Vec<SimResult>,
}

#[derive(Debug, Default, PartialEq)]
pub struct SimResult {
    /// Number of nodes under attack
    pub num_nodes: usize,
    /// Successful payments
    pub num_successful: usize,
    pub num_failed: usize,
}

impl SimOutput {}
impl SimResult {
    pub(crate) fn from_simlib_results(sim_results: simlib::SimResult, num_nodes: usize) -> Self {
        Self {
            num_nodes,
            num_successful: sim_results.num_succesful,
            num_failed: sim_results.num_failed,
        }
    }
}

#[cfg(test)]
mod tests {}
