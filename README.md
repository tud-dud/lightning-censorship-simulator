# simulator

![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
[![CI](https://github.com/p2p-research-tools/lightning-censorship-simulator/actions/workflows/test.yml/badge.svg)](https://github.com/p2p-research-tools/lightning-censorship-simulator/actions/workflows/test.yml)
[![codecov](https://codecov.io/gh/p2p-research-tools/lightning-censorship-simulator/branch/main/graph/badge.svg?token=QZH345MHCJ)](https://codecov.io/gh/p2p-research-tools/lightning-censorship-simulator)
[![dependency status](https://deps.rs/repo/github/p2p-research-tools/lightning-censorship-simulator/status.svg)](https://deps.rs/repo/github/p2p-research-tools/lightning-censorship-simulator)

This is a set of binaries to simulate payment delivery in the Lightning network
under various attack scenarios or analyse the network-level topology.

## Build

Compile all members of the project:

`cargo build --release`

Run all unit tests:

`cargo test --release`

## simulator

The binary reconstructs the network topology using an input graph, maps nodes to
ASNs and uses the
[lightning-simulator](https://github.com/p2p-research-tools/lightning-simulator)
to simulate payment delivery in the network.
The tool simulates payment failure under different attack scenarios.

  <details>
    <summary>usage</summary>

       target/release/simulator [OPTIONS] <GRAPH_FILE> [VERBOSE]

       Arguments:
         <GRAPH_FILE>  Path to JSON ile describing topology
         [VERBOSE]

       Options:
         -l, --log <LOG_LEVEL>                [default: info]
         -o, --out <OUTPUT_DIR>               Path to directory in which the results will be stored
         -a, --amount <AMOUNT>                The payment volume (in sat) we are trying to route
         -r, --run <RUN>                      Set the seed for the simulation [default: 19]
         -g, --graph-source <GRAPH_TYPE>      [default: lnd] [possible values: lnd, lnr]
         -p, --payments <NUM_PAIRS>           Number of src/dest pairs to use in the simulation [default: 1000]
         -n, --num-as <NUM_ADV_AS>            The number of adversarial ASs to simulate (top-n) [default: 5]
         -s, --as-strategy <AS_SEL_STRATEGY>  AS selection strategy. 0 for number of nodes and 1 for number of channels [default: 1]
         -h, --help                           Print help
         -V, --version                        Print version 
  </details>

## as_node_degree

The binary reads the channel graph and maps each to node with a public address
to its ASN.
The output is a CSV file with two columns per node -- its ASN and degree (number
of channels).

*NB: Nodes with only a Tor address are assigned ASN 0.*

  <details>
    <summary>usage</summary>

        target/release/as_node_degree [OPTIONS] <GRAPH_FILE> [VERBOSE]

        Arguments:
          <GRAPH_FILE>  Path to JSON file describing topology
          [VERBOSE]

        Options:
          -l, --log <LOG_LEVEL>            [default: info]
          -o, --out <OUTPUT_PATH>          Path to directory where the results will be stored
          -g, --graph-source <GRAPH_TYPE>  [default: lnd] [possible values: lnd, lnr]
          -u, --overwrite
          -h, --help                       Print help
          -V, --version                    Print version
  </details>

## intra_as_channels

The binary reads the channel graph, maps each to node with a public address
to its ASN and counts the number of channels the node has to other nodes in its
ASN.
The output is a CSV file with three columns per AS -- its ASN, the total number
of intra-AS channels and the total number of inter-AS channels.

*NB: Nodes with only a Tor address are assigned ASN 0.*

  <details>
    <summary>usage</summary>

        Usage: target/release/intra_channels [OPTIONS] <GRAPH_FILE> [VERBOSE]

        Arguments:
          <GRAPH_FILE>  Path to JSON file describing topology
          [VERBOSE]

        Options:
          -l, --log <LOG_LEVEL>            [default: info]
          -o, --out <OUTPUT_PATH>          Path to CSV file where the results should be written to
          -g, --graph-source <GRAPH_TYPE>  [default: lnd] [possible values: lnd, lnr]
          -u, --overwrite
          -h, --help                       Print help
          -V, --version                    Print version
  </details>
