use clap::Parser;
use csv::Writer;
use log::{error, info, LevelFilter};
use simlib::graph::Graph;
use simulator::AsIpMap;
use std::{collections::HashMap, error::Error, path::PathBuf};

#[derive(clap::Parser)]
#[command(name = "intra-channels", version, about)]
struct Cli {
    /// Path to JSON ile describing topology
    graph_file: PathBuf,
    #[arg(long = "log", short = 'l', default_value = "info")]
    log_level: LevelFilter,
    /// Path to directory in which the results will be stored
    #[arg(long = "out", short = 'o')]
    output_path: Option<PathBuf>,
    #[arg(long = "graph-source", short = 'g', default_value = "lnd")]
    graph_type: network_parser::GraphSource,
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
        PathBuf::from("ln-intra-channels.csv")
    };
    info!("Topology analysis will be written to {:#?}.", output_path);
    let as_ip_map = AsIpMap::new(&graph, true);
    let ratios = get_intra_as_channels_ratio(&as_ip_map.as_to_nodes, &graph);
    write_to_csv_file(&ratios, &output_path, args.overwrite).unwrap();
    info!("CSV successfully written to {:#?}.", output_path);
}

fn get_intra_as_channels_ratio(
    as_to_nodes: &HashMap<u32, Vec<String>>,
    graph: &Graph,
) -> HashMap<u32, Vec<f32>> {
    let mut per_node_ratio = HashMap::new();
    let find_key_for_value = |map: &HashMap<u32, Vec<_>>, value: &String| -> Option<u32> {
        map.iter().find_map(|(key, val)| {
            if val.contains(value) {
                Some(*key)
            } else {
                None
            }
        })
    };

    for (asn, nodes) in as_to_nodes.iter() {
        per_node_ratio.insert(*asn, vec![]);
        for node in nodes {
            if let Some(edges) = graph.get_edges_for_node(node) {
                let total = edges.len();
                if total.eq(&0) {
                    // shouldnt happen
                    break;
                }
                let mut same_asn = 0;
                for e in edges.iter() {
                    if let Some(dst_asn) = find_key_for_value(as_to_nodes, &e.destination) {
                        if dst_asn == *asn {
                            same_asn += 1;
                        }
                    }
                }
                let ratio = f32::trunc((same_asn as f32 / total as f32) * 100.0) / 100.0;
                per_node_ratio.entry(*asn).and_modify(|r| r.push(ratio));
            }
        }
    }
    per_node_ratio
}

fn write_to_csv_file(
    data: &HashMap<u32, Vec<f32>>,
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
        writer.serialize(("asn", "ratio"))?;
        for (asn, rates) in data.iter() {
            for r in rates {
                writer.serialize((asn, r))?;
            }
            writer.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_parser::GraphSource::*;
    use std::path::Path;

    #[test]
    fn intra_channels_rate() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let include_tor = true;
        let as_ip_map = AsIpMap::new(&graph, include_tor);
        let actual = get_intra_as_channels_ratio(&as_ip_map.as_to_nodes, &graph);
        let expected = HashMap::from([(24940, vec![0.5, 1.0]), (797, vec![0.5, 1.0])]);
        assert_eq!(actual.len(), expected.len());
        for a in actual {
            let e = expected.get(&a.0).unwrap();
            assert_eq!(a.1.len(), e.len());
            for expected_ratio in e {
                assert!(a.1.contains(expected_ratio));
            }
        }
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/trivial_connected_lnd.json"),
                Lnd,
            )
            .unwrap(),
            Lnd,
        );
        let as_ip_map = AsIpMap::new(&graph, include_tor);
        let actual = get_intra_as_channels_ratio(&as_ip_map.as_to_nodes, &graph);
        let expected = HashMap::from([(24940, vec![0.5, 0.5]), (797, vec![0.])]);
        assert_eq!(actual.len(), expected.len());
        for a in actual {
            let e = expected.get(&a.0).unwrap();
            assert_eq!(a.1.len(), e.len());
            for expected_ratio in e {
                assert!(a.1.contains(expected_ratio));
            }
        }
    }
}
