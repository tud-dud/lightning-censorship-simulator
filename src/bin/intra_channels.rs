use clap::Parser;
use csv::Writer;
use log::{error, info, LevelFilter};
use simulator::AsIpMap;
use std::{collections::HashMap, error::Error, path::PathBuf};

#[derive(clap::Parser)]
#[command(name = "intra-channels", version, about)]
struct Cli {
    /// Path to JSON file describing topology
    graph_file: PathBuf,
    #[arg(long = "log", short = 'l', default_value = "info")]
    log_level: LevelFilter,
    /// Path to CSV file where the results will be stored
    #[arg(long = "out", short = 'o')]
    output_path: Option<PathBuf>,
    #[arg(long = "graph-source", short = 'g', default_value = "lnd")]
    graph_type: network_parser::GraphSource,
    /// Overwrite the existing file, if it exists
    #[arg(short = 'u', long = "overwrite")]
    overwrite: bool,
    verbose: bool,
}

fn main() {
    let args = Cli::parse();
    let log_level = args.log_level;
    env_logger::builder().filter_level(log_level).init();
    let graph_source = args.graph_type;
    let g = network_parser::Graph::from_json_file(
        std::path::Path::new(&args.graph_file),
        graph_source.clone(),
    );
    let graph = match g {
        Ok(graph) => simlib::core_types::graph::Graph::to_sim_graph(&graph, graph_source),
        Err(e) => {
            error!("Error in graph file {}. Exiting.", e);
            std::process::exit(-1)
        }
    };
    let output_path = if let Some(output_path) = args.output_path {
        output_path
    } else {
        PathBuf::from("ln-intra-inter-channels.csv")
    };
    info!(
        "Topology analysis results will be written to {:#?}.",
        output_path
    );
    let as_ip_map = AsIpMap::new(&graph, true);
    let sums = as_ip_map.get_sum_of_as_channels(&graph);
    let ratios = as_ip_map.get_intra_inter_ratio_of_as_channels_capacity(&graph);
    write_to_csv_file(&sums, &ratios, &output_path, args.overwrite).unwrap();
    info!("CSV successfully written to {:#?}.", output_path);
}

fn write_to_csv_file<T: serde::ser::Serialize, U: serde::ser::Serialize + Clone>(
    data: &HashMap<u32, (T, T)>,
    data2: &HashMap<u32, (U, U)>,
    output_path: &PathBuf,
    overwrite_allowed: bool,
) -> Result<(), Box<dyn Error>> {
    if !overwrite_allowed && output_path.exists() {
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Output file exists, refusing to overwrite.",
        )))
    } else {
        let mut writer = Writer::from_path(output_path)?;
        writer.serialize(("asn", "intra-sum", "inter-sum", "intra-cap", "inter-cap"))?;
        for (asn, (num_intra, num_inter)) in data.iter() {
            let d2 = data2.get(asn).unwrap();
            let (ratio_intra, ratio_inter) = (d2.clone().0, d2.clone().1);
            writer.serialize((asn, num_intra, num_inter, ratio_intra, ratio_inter))?;
            writer.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use csv::{Reader, StringRecord};
    use tempfile::NamedTempFile;

    #[test]
    fn persist() {
        let sums = HashMap::from([(0, (1, 2))]);
        let ratios = HashMap::from([(0, (0.0, 1.0))]);
        let file = NamedTempFile::new().expect("Error opening tempfile");
        let overwrite = false;
        assert!(write_to_csv_file(&sums, &ratios, &PathBuf::from(file.path()), overwrite).is_err());
        let overwrite = true;
        assert!(write_to_csv_file(&sums, &ratios, &PathBuf::from(file.path()), overwrite).is_ok());
        let mut reader = Reader::from_path(file.path()).unwrap();
        assert_eq!(
            *reader.headers().unwrap(),
            StringRecord::from(vec![
                "asn",
                "intra-sum",
                "inter-sum",
                "intra-cap",
                "inter-cap"
            ])
        );
        for record in reader.records() {
            assert_eq!(
                record.unwrap(),
                StringRecord::from(vec!["0", "1", "2", "0.0", "1.0"])
            );
        }
    }
}
