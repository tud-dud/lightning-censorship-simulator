use log::{error, info};
use serde::Serialize;
use simlib::io::PaymentInfo;
use std::{
    error::Error,
    fs::{self, File},
    path::PathBuf,
};

use crate::net::Asn;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report(pub u64, pub Vec<SimOutput>);

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimOutput {
    pub amt_sat: usize,
    pub total_num_payments: usize,
    /// Includes baseline results when no nodes are under attack
    pub attack_results: Vec<AttackSim>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackSim {
    pub asn: Asn,
    pub sim_results: Vec<SimResult>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimResult {
    /// Number of nodes under attack
    pub num_nodes_under_attack: usize,
    /// Successful payments
    pub num_successful: usize,
    pub num_failed: usize,
    pub payments: Vec<PaymentInfo>,
}

impl Report {
    pub fn write_to_file(&self, path: PathBuf) -> Result<(), Box<dyn Error>> {
        if fs::create_dir_all(&path).is_ok() {
            self.to_json_file(path)?;
        } else {
            error!("Directory creation failed.");
        }
        Ok(())
    }

    fn to_json_file(&self, output_path: PathBuf) -> Result<(), Box<dyn Error>> {
        let run_as_string = format!("{}{:?}", "simulation-run", self.0);
        let mut file_output_path = output_path;
        file_output_path.push(format!("{}{}", run_as_string, ".json"));
        let file = File::create(file_output_path.clone()).expect("Error creating file.");
        serde_json::to_writer_pretty(file, self).expect("Error writing to JSON file.");
        info!(
            "Simulation output written to {}.",
            file_output_path.display()
        );
        Ok(())
    }
}
impl SimResult {
    pub(crate) fn from_simlib_results(sim_results: simlib::SimResult, num_nodes: usize) -> Self {
        let mut payments: Vec<PaymentInfo> = sim_results
            .successful_payments
            .iter()
            .map(PaymentInfo::from_payment)
            .collect();
        payments.extend(
            sim_results
                .failed_payments
                .iter()
                .map(PaymentInfo::from_payment),
        );
        Self {
            num_nodes_under_attack: num_nodes,
            num_successful: sim_results.num_succesful,
            num_failed: sim_results.num_failed,
            payments,
        }
    }
}

#[cfg(test)]
mod tests {}
