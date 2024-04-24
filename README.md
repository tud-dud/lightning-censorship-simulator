# simulator

![MIT](https://img.shields.io/badge/license-MIT-blue.svg)

A set of binaries to simulate payment delivery in the Lightning network under
various attack scenarios or analyse the network-level topology.

## Build

Build all members of the project:

`cargo build --release`

## simulator

The bianry reconstructs the network topology using an input graph, maps nodes to
ASNs and implements payment delivery in the network. The tool simulates payment
failure scenarios under different attack scenarios.

  <details>
    <summary>simulator</summary>

    ```
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
    ```
  </details>
