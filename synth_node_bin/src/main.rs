//! A synthetic node binary can be used to interact with the node in the
//! background from a different runtime environment.
//!
//! This is a quick solution - in future Ziggurat projects it will be done differently.
//!
//! Remaining worklist (probably not going to be implemented in this repo):
//!   - Add an argument option to choose specific action in order to support
//!     different synthetic node binary implementations at once. A few examples:
//!     ```
//!        ./synthetic_node_bin --action=A    // Runs an idle/friendly synthetic node
//!        ./synthetic_node_bin --action=B    // Runs a wild synthetic node which does something funny
//!     ```
use std::{net::SocketAddr, process::ExitCode};

use action::{ActionHandler, ActionType};
use anyhow::Result;
use clap::Parser;
use ziggurat_zcash::tools::synthetic_node::SyntheticNode;

mod action;

/// A synthetic node which can connect to the XRPL node and preform some
/// actions independently.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct CmdArgs {
    /// An address of the node in the <ip>:<port> format.
    #[arg(short = 'i', long)]
    node_addr: Option<SocketAddr>,

    /// Always reconnect in the case the connection fails - synthetic node never dies.
    #[arg(short = 's', long, default_value_t = false)]
    stubborn: bool,

    /// Enable tracing.
    #[arg(short = 't', long, default_value_t = false)]
    tracing: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = CmdArgs::parse();

    let node_addr = if let Some(addr) = args.node_addr {
        addr
    } else {
        eprintln!("Node address should be provided.");
        return ExitCode::FAILURE;
    };

    if args.tracing {
        println!("Enabling tracing.");
        use tracing_subscriber::{fmt, EnvFilter};

        fmt()
            .with_test_writer()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }

    loop {
        println!("Starting a synthetic node.");

        if let Err(e) = run_synth_node(node_addr).await {
            eprintln!("The synthetic node stopped: {e:?}.");
        }

        // Use the stubborn option to run the synth node infinitely.
        if !args.stubborn {
            break;
        }
    }

    ExitCode::SUCCESS
}

async fn run_synth_node(node_addr: SocketAddr) -> Result<()> {
    // Select action.
    let action = ActionHandler::new(ActionType::SendGetAddrAndForeverSleep);

    // Create a synthetic node and enable handshaking.
    let mut synth_node = SyntheticNode::builder()
        .with_network_config(action.cfg.network_cfg.clone())
        .with_full_handshake()
        .with_message_filter(action.cfg.msg_filter.clone())
        .build()
        .await
        .unwrap();

    // Perform the handshake.
    synth_node.connect(node_addr).await?;

    // Run the wanted action with the node.
    action.execute(&mut synth_node, node_addr).await?;

    // Stop the synthetic node.
    synth_node.shut_down().await;

    Ok(())
}
