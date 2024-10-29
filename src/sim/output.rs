use log::{error, info};
use serde::Serialize;
use simlib::io::PaymentInfo;
use std::{
    error::Error,
    fs::{self, File},
    path::PathBuf,
};

use crate::PacketDropStrategy;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub run: u64,
    pub censor_sim_output: Vec<SimOutput>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimOutput {
    pub amt_sat: usize,
    pub total_num_payments: usize,
    pub per_strategy_results: Vec<PerStrategyResults>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerStrategyResults {
    pub strategy: PacketDropStrategy,
    /// Includes baseline results when no nodes are under attack
    pub attack_results: Vec<AttackSim>,
}
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackSim {
    pub asn: String,
    pub sim_results: Vec<SimResult>, // the first list is for the baseline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_sim_accuracy: Option<PerSimAccuracy>, // not present in baseline or when all are
                                     // dropped so we only have one
}

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SimResult {
    /// Number of nodes under attack which we only use for the baseline
    pub num_nodes_under_attack: usize,
    /// Successful payments
    pub num_successful: usize,
    pub num_failed: usize,
    pub payments: Vec<PaymentInfo>,
    pub payments_dist: Option<PaymentsDist>,
    pub involved_asns: Option<InvolvedAsns>,
}

/// A list containing the number of involved ASNs for each successful payment
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvolvedAsns {
    pub num_asns: Vec<usize>,
}

/// Number of correctly and falsely identified intra-AS payments for PacketDropStrategy::Intra
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerSimAccuracy {
    pub tpos: usize,
    pub fpos: usize,
    pub fneg: usize,
}

/// Share of {Successful | failed} inter-AS payments in the simulation. Only present for the baseline simulation
/// 100.0 - (inter_succ + inter_failed) = intra-AS payments
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentsDist {
    pub inter_succ: f32,
    pub inter_failed: f32,
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
        let run_as_string = format!("{}{:?}", "simulation-run", self.run);
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
    pub fn from_simlib_results(
        sim_results: simlib::SimResult,
        num_nodes: usize,
        payments_dist: Option<PaymentsDist>,
        involved_asns: Option<InvolvedAsns>,
    ) -> Self {
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
            payments_dist,
            involved_asns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simlib::{payment::Payment, CandidatePath};
    use std::collections::VecDeque;
    use tempfile::TempDir;

    #[test]
    fn convert_simulation_results() {
        let mut successful_payment =
            Payment::new(0, String::from("dina"), String::from("bob"), 1, None);
        let mut path = simlib::Path::new(String::from("dina"), String::from("bob"));
        path.hops = VecDeque::from([
            ("dina".to_string(), 0, 0, "".to_string()),
            ("chan".to_string(), 0, 0, "c".to_string()),
            ("bob".to_string(), 0, 0, "".to_string()),
        ]);
        successful_payment.succeeded = true;
        successful_payment.used_paths = vec![CandidatePath::new_with_path(path)];
        let sim_result = simlib::SimResult {
            num_succesful: 2,
            num_failed: 1,
            total_num: 3,
            successful_payments: vec![successful_payment.clone(), successful_payment],
            failed_payments: vec![Payment::new(
                1,
                String::from("chan"),
                String::from("bob"),
                1,
                None,
            )],
            ..Default::default()
        };
        let actual = SimResult::from_simlib_results(sim_result.clone(), 0, None, None);
        let mut payments: Vec<PaymentInfo> = sim_result
            .successful_payments
            .iter()
            .map(PaymentInfo::from_payment)
            .collect();
        payments.extend(
            sim_result
                .failed_payments
                .iter()
                .map(PaymentInfo::from_payment),
        );
        let expected = SimResult {
            num_nodes_under_attack: 0,
            num_successful: 2,
            num_failed: 1,
            payments,
            payments_dist: None,
            involved_asns: None,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn write() {
        let path = TempDir::new().expect("Error opening tempfile");
        let report = Report::default();
        assert!(report.write_to_file(PathBuf::from(path.path())).is_ok());
    }
}
