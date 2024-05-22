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
    /// Path to CSV file where the results should be written to
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
    info!("Topology analysis will be written to {:#?}.", output_path);
    let sums = AsIpMap::new(&graph, true).get_sum_of_as_channels(&graph);
    write_to_csv_file(&sums, &output_path, args.overwrite).unwrap();
    info!("CSV successfully written to {:#?}.", output_path);
}

fn write_to_csv_file(
    data: &HashMap<u32, (u32, u32)>,
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
        writer.serialize(("asn", "intra", "inter"))?;
        for (asn, (num_intra, num_inter)) in data.iter() {
            writer.serialize((asn, num_intra, num_inter))?;
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
        let file = NamedTempFile::new().expect("Error opening tempfile");
        let overwrite = false;
        assert!(write_to_csv_file(&sums, &PathBuf::from(file.path()), overwrite).is_err());
        let overwrite = true;
        assert!(write_to_csv_file(&sums, &PathBuf::from(file.path()), overwrite).is_ok());
        let mut reader = Reader::from_path(file.path()).unwrap();
        assert_eq!(
            *reader.headers().unwrap(),
            StringRecord::from(vec!["asn", "intra", "inter"])
        );
        for record in reader.records() {
            assert_eq!(record.unwrap(), StringRecord::from(vec!["0", "1", "2"]));
        }
    }
}
