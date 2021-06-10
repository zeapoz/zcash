use crate::{
    helpers::{initiate_handshake, is_rejection_error, is_termination_error},
    protocol::message::{filter::MessageFilter, Message},
    setup::{
        config::new_local_addr,
        node::{Action, Node},
    },
};

#[derive(Debug)]
enum PeerEvent {
    Rejected,
    Terminated,
    Connected,
    HandshakeError(std::io::Error),
    UnexpectedMessage(Box<Message>),
    ReadError(std::io::Error),
}

#[derive(Default, Debug)]
struct ConnectionStats {
    /// count of successfuly handshaken connections
    pub success: u16,
    /// count of connections rejected pre-handshake
    pub rejected: u16,
    /// count of connections terminated post-handshake
    pub terminated: u16,
    /// running count of active connections (approximate since nodes
    /// are executing concurrently)
    pub active_count: Vec<u16>,
}

// implement display to display the max of count
impl std::fmt::Display for ConnectionStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "success: {}, rejected: {}, terminated: {}, max_active: {}",
            self.success,
            self.rejected,
            self.terminated,
            self.active_count.iter().max().unwrap_or(&0),
        )
    }
}

#[tokio::test]
async fn incoming_active_connections() {
    // ZG-PERFORMANCE-002
    //
    // The node sheds or rejects connections when necessary.
    //
    //  1. Start a node with max_peers set to `N`
    //  2. Initiate connections from `M > N` peer nodes
    //  3. Expect only `N` to be active at a time
    //
    // This test currently fails for both zebra and zcashd.
    //
    // ZCashd: Accepts only N-8 connections, possibly due to reserving
    //         connections for the hardcoded seeds (TBC).
    //         Example runs (with N=50):
    //
    //                  ╔════╦════════╦══════════╦═══════════╦═════════════╦═════════════╦════════╗
    //                  ║ N  ║   M    ║ Accepted ║ Rejected  ║ Terminated  ║ Max active  ║  Time  ║
    //                  ╠════╬════════╬══════════╬═══════════╬═════════════╬═════════════╬════════╣
    //                  ║ 50 ║    100 ║       42 ║        58 ║           0 ║          42 ║  3.73s ║
    //                  ║ 50 ║  1_000 ║       42 ║       958 ║           0 ║          42 ║  2.33s ║
    //                  ║ 50 ║  5_000 ║       42 ║     4_958 ║           0 ║          42 ║  2.83s ║
    //                  ║ 50 ║ 10_000 ║       42 ║     9_958 ║           0 ║          42 ║  3.67s ║
    //                  ║ 50 ║ 15_000 ║       42 ║    14_958 ║           0 ║          42 ║  6.19s ║
    //                  ║ 50 ║ 20_000 ║       42 ║    19_958 ║           0 ║          42 ║ 20.13s ║
    //                  ╚════╩════════╩══════════╩═══════════╩═════════════╩═════════════╩════════╝
    //
    // Zebra: Ignores the set limit, and does not appear to have an actual limit
    //        set. Example runs (with N=50):
    //
    //                  ╔════╦════════╦══════════╦═══════════╦═════════════╦═════════════╦════════╗
    //                  ║ N  ║   M    ║ Accepted ║ Rejected  ║ Terminated  ║ Max active  ║  Time  ║
    //                  ╠════╬════════╬══════════╬═══════════╬═════════════╬═════════════╬════════╣
    //                  ║ 50 ║    100 ║      100 ║         0 ║           0 ║         100 ║  0.59s ║
    //                  ║ 50 ║  1_000 ║    1_000 ║         0 ║           0 ║       1_000 ║  1.17s ║
    //                  ║ 50 ║  5_000 ║    4_962 ║        38 ║           0 ║       4_962 ║  3.55s ║
    //                  ║ 50 ║ 10_000 ║    9_777 ║       223 ║           0 ║       9_777 ║  8.96s ║
    //                  ║ 50 ║ 15_000 ║   13_782 ║     1_218 ║      12_077 ║      12_651 ║ 66.57s ║
    //                  ║ 50 ║ 20_000 ║    4_255 ║    15_745 ║           1 ║       4_255 ║ 34.06s ║
    //                  ╚════╩════════╩══════════╩═══════════╩═════════════╩═════════════╩════════╝
    //
    // The example runs are pretty representative. In particular, zebra starts exhibiting weird
    // behaviour at 15_000.

    /// maximum peers to configure node with
    const MAX_PEERS: usize = 50;
    /// number of test peer nodes to spin-up
    const TEST_PEER_COUNT: usize = 100;

    // channel for peer event management (ensure buffer is more than large enough)
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<PeerEvent>(TEST_PEER_COUNT * 3);

    // start node
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection)
        .max_peers(MAX_PEERS)
        .start()
        .await;

    // start peer event manager
    let event_manager = tokio::spawn(async move {
        let mut stats = ConnectionStats::default();

        loop {
            match event_rx.recv().await.unwrap() {
                PeerEvent::Rejected => stats.rejected += 1,
                PeerEvent::Terminated => {
                    stats.terminated += 1;
                    // Since a connection cannot be terminated without first having connected,
                    // there will always be a `last` item here.
                    let prev_count = *stats.active_count.last().unwrap();
                    stats.active_count.push(prev_count - 1);
                }
                PeerEvent::Connected => {
                    stats.success += 1;
                    let prev_count = *stats.active_count.last().unwrap_or(&0);
                    stats.active_count.push(prev_count + 1);
                }
                PeerEvent::HandshakeError(err) => panic!("Handshake error: {:?}", err),
                PeerEvent::UnexpectedMessage(msg) => panic!("Unexpected message: {:?}", msg),
                PeerEvent::ReadError(err) => panic!("Read error:\n{:?}", err),
            }

            // We are done if all peer connections have either been accepted or rejected
            if (stats.success + stats.rejected) as usize == TEST_PEER_COUNT {
                break;
            }
        }
        stats
    });

    // start peer nodes
    let mut peer_handles = Vec::with_capacity(TEST_PEER_COUNT);
    let mut peer_exits = Vec::with_capacity(TEST_PEER_COUNT);

    for _ in 0..TEST_PEER_COUNT {
        let node_addr = node.addr();
        let peer_send = event_tx.clone();

        let (exit_tx, exit_rx) = tokio::sync::oneshot::channel::<()>();
        peer_exits.push(exit_tx);

        peer_handles.push(tokio::spawn(async move {
            // Establish peer connection
            let mut stream = match initiate_handshake(node_addr).await {
                Ok(stream) => {
                    let _ = peer_send.send(PeerEvent::Connected).await;
                    stream
                }
                Err(err) if is_rejection_error(&err) => {
                    let _ = peer_send.send(PeerEvent::Rejected).await;
                    return;
                }
                Err(err) => {
                    let _ = peer_send.send(PeerEvent::HandshakeError(err)).await;
                    return;
                }
            };

            // Keep connection alive by replying to incoming Pings etc, until instructed to exit or
            // connection is terminated (or something unexpected occurs).
            let filter = MessageFilter::with_all_auto_reply();
            tokio::select! {
                _ = exit_rx => {},
                result = filter.read_from_stream(&mut stream) => {
                    match result {
                        Ok(message) => {
                            let _ = peer_send
                                .send(PeerEvent::UnexpectedMessage(message.into()))
                                .await;
                        }
                        Err(err) if is_termination_error(&err) => {
                            let _ = peer_send.send(PeerEvent::Terminated).await;
                        }
                        Err(err) => {
                            let _ = peer_send.send(PeerEvent::ReadError(err)).await;
                        }
                    }
                }
            }
        }));
    }

    // Wait for event manager to complete its tally
    let stats = event_manager.await.unwrap();

    // Send stop signal to peer nodes. We ignore the possible error
    // result as this will occur with peers that have already exited.
    for stop in peer_exits {
        let _ = stop.send(());
    }

    // Wait for peers to complete
    for handle in peer_handles {
        handle.await.unwrap();
    }

    node.stop().await;

    // We currently assume no accepted peer connection gets terminated.
    // This can technically occur, but currently doesn't as all our peer
    // nodes are created equal (so the node doesn't drop our existing peers in
    // favor of new connections).
    //
    // If this is no longer true, then we need to start tracking the statistics
    // over time instead of just totals. So this is a sanity check to ensure that
    // assumption still applies.
    assert_eq!(stats.terminated, 0, "Stats: {}", stats);
    // We expect to have `MAX_PEERS` connections. This is only true if
    // `stats.terminated == 0`.
    assert_eq!(stats.success as usize, MAX_PEERS, "Stats: {}", stats);
    // The rest of the peers should be rejected.
    assert_eq!(
        stats.rejected as usize,
        TEST_PEER_COUNT - MAX_PEERS,
        "Stats: {}",
        stats
    );
}
