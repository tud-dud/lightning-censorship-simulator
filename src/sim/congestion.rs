use std::collections::HashMap;

use super::{output::*, SimBuilder};
use crate::{net::Asn, AsIpMap};
use log::{info, trace};
use simlib::{graph::Graph, Edge, PaymentParts, RoutingMetric, Simulation, ID};

impl SimBuilder {
    /// we remove the affected channels from the graph and then run the same set of payments
    pub(crate) fn apply_congested_channels_strategy(
        sim_result: simlib::SimResult,
        asn: Asn,
        as_ip_map: &AsIpMap,
        congestion_rate: usize,
        graph: &Graph,
    ) -> (simlib::SimResult, Option<PerSimAccuracy>) {
        let mut g = graph.clone();
        g.set_edges(Self::disable_as_channels(
            graph,
            congestion_rate,
            asn,
            as_ip_map,
        ));
        let payment_pairs = sim_result
            .successful_payments
            .into_iter()
            .map(|p| (p.source, p.dest))
            .chain(
                sim_result
                    .failed_payments
                    .into_iter()
                    .map(|p| (p.source, p.dest)),
            );
        let mut sim = Simulation::new(
            sim_result.run,
            g,
            sim_result.amount,
            RoutingMetric::MinFee,
            PaymentParts::Split,
            Some(vec![0]),
            &[],
        );
        info!(
            "Starting AS {} congestion simulation of {} sat.",
            asn, sim_result.amount
        );
        let results = sim.run(payment_pairs, None, false);
        (results, None)
    }

    /// We mark channels as congested if they are inter-AS, i.e., if either src == asn && dst !=
    /// asn or vice versa
    fn disable_as_channels(
        graph: &Graph,
        congestion_rate: usize,
        asn: Asn,
        as_ip_map: &AsIpMap,
    ) -> HashMap<ID, Vec<Edge>> {
        let mut edges = graph.get_edges().clone();
        let mut as_channels = vec![];
        for edges in graph.get_edges().values() {
            for channel in edges {
                let src_asn = as_ip_map
                    .get_asn_for_node(&channel.source)
                    .unwrap_or_default();
                let dst_asn = as_ip_map
                    .get_asn_for_node(&channel.destination)
                    .unwrap_or_default();
                if src_asn == asn && dst_asn != asn || dst_asn == asn && src_asn != asn {
                    trace!(
                        "Channel {} between {} and {} is now congested.",
                        channel.channel_id,
                        channel.source,
                        channel.destination
                    );
                    as_channels.push(channel);
                }
            }
        }
        as_channels.sort_unstable_by(|a, b| b.capacity.partial_cmp(&a.capacity).unwrap());
        let num_congested = (congestion_rate as f32 / 100.0) as usize * as_channels.len();
        info!(
            "Marking {}% (={}) of AS {}'s channels as congested.",
            congestion_rate, num_congested, asn
        );
        // we set the capacity to 0 which makes them unusable for payments
        for (i, chan) in as_channels.iter().enumerate() {
            if i == num_congested {
                break;
            } else if let Some(c) = edges
                .get_mut(&chan.source)
                .and_then(|e| e.iter_mut().find(|x| x.channel_id == chan.channel_id))
            {
                c.capacity = 0;
            }
        }
        edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use network_parser::GraphSource::*;
    use simlib::graph::Graph;
    use std::path::Path;

    #[test]
    fn unusable_as_channels() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let as_ip_map = AsIpMap::new(&graph, false);
        let asn = 797;
        let congestion_rate = 100;
        let actual = SimBuilder::disable_as_channels(&graph, congestion_rate, asn, &as_ip_map);
        assert_eq!(actual.len(), graph.get_edges().len());
        for edges in actual.values() {
            for channel in edges {
                if channel.source == "bob" && channel.destination == "chan"
                    || channel.destination == "bob" && channel.source == "chan"
                {
                    assert_eq!(channel.capacity, 0);
                }
            }
        }
    }

    #[test]
    fn sim_congested_asn() {
        let graph = Graph::to_sim_graph(
            &network_parser::Graph::from_json_file(
                &Path::new("test_data/lnbook_example_lnr.json"),
                Lnresearch,
            )
            .unwrap(),
            Lnresearch,
        );
        let as_ip_map = AsIpMap::new(&graph, false);
        let mut sim = SimBuilder::new(
            4711,
            &graph,
            1000,
            0,
            crate::AsSelectionStrategy::MaxChannels,
        );
        let pairs = [
            ("alice".to_string(), "bob".to_string()),
            ("chan".to_string(), "dina".to_string()),
            ("alice".to_string(), "dina".to_string()),
        ];
        let asn = 797;
        let congestion_rate = 0;
        let baseline = sim.simulate(pairs.into_iter());
        let (actual, _) = SimBuilder::apply_congested_channels_strategy(
            baseline.clone(),
            asn,
            &as_ip_map,
            congestion_rate,
            &graph,
        );
        assert_eq!(baseline, actual);
        let congestion_rate = 100;
        let (actual, _) = SimBuilder::apply_congested_channels_strategy(
            baseline.clone(),
            asn,
            &as_ip_map,
            congestion_rate,
            &graph,
        );
        assert_eq!(actual.successful_payments.len(), 2);
        assert_eq!(actual.successful_payments[0].source, "alice".to_owned());
        assert_eq!(actual.successful_payments[0].dest, "bob".to_owned());
        assert_eq!(actual.successful_payments[1].source, "chan".to_owned());
        assert_eq!(actual.successful_payments[1].dest, "dina".to_owned());
    }
}
