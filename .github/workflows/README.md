# Ziggurat CI/CD

This documentation details information on how this implementation handles CI/CD, and can be used as a reference for setting up your own CI/CD pipeline with Ziggurat.

Currently the Ziggurat CI/CD pipeline includes three concurrent workflows that run daily, these are the test suites for `zcashd`, `zebra` and the network crawler; and another workflow that runs on commits and pull requests to the main branch to check that formatting rules are followed and that checks are passed. Each workflow is described in more detail below: 

## Test Suite

The test suite workflows can be broken down into the following 5 steps:
1. Build a selected node from source.
2. Compile Ziggurat unit tests.
3. Create the Ziggurat config file. 
4. Run the Ziggurat tests executable.
5. Process the results.

## Network Crawler

The network crawler workflow can be broken down into the following 4 steps:
1. Build a `zcashd` node from source.
2. Run the crawler binary with the compiled node as the network entry point.
3. Wait 30 minutes, then query metrics via RPC and kill the running crawler.
4. Process the results.

Details on how to run the crawler, including the required arguments and how to work with the RPC, can be found [here](../../src/tools/crawler/README.md).

### Crunchy

Following the procuring of crawler results, the crawler will call another workflow, crunchy, that computes additional network metrics. This process can roughly be described as:
1. Clone the [Crunchy](https://github.com/runziggurat/crunchy) repo.
2. Generate samples from the latest crawler results.

## Check and Lint

The check and lint workflow currently performs a set of six different checks, these are:
* **Core checks** (inherited from [`ziggurat-core`](https://github.com/runziggurat/ziggurat-core)):
1. check - `cargo check --all-targets`.
2. fmt - `cargo fmt --all -- --check`.
3. clippy - `cargo clippy --all-targets -- -D warnings`.
4. sort - `cargo-sort --check --workspace`.

* **Extra checks**:
5. test-ignored - `cargo test -- --test-threads=1 --ignored --skip dev`.
6. check-crawler - `cargo check --features=crawler`.

For details regarding implementation and how to extend these tests, please refer to [this section](https://github.com/runziggurat/ziggurat-core#Nix) of the `ziggurat-core` documentation.

## Workflow References

- [Test Suite (`zcashd`)](./zcashd-nightly.yml)
- [Test Suite (`zebra`)](./zebra.yml)
- [Network Crawler](./crawler.yml)
- [Build `zcashd`](./build-zcashd.yml)

### Ziggurat Core Workflows

Most workflows will also reference a set of core utilities that are used throughout the Ziggurat ecosystem. These can all be found in the `ziggurat-core` repository, which can be found [here](https://github.com/runziggurat/ziggurat-core/blob/main/.github/workflows/README.md).
