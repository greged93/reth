//! Example for how hook into the polygon p2p network
//!
//! Run with
//!
//! ```sh
//! cargo run -p polygon-p2p
//! ```
//!
//! This launches a regular reth node overriding the engine api payload builder with our custom.
//!
//! Credits to: <https://merkle.io/blog/modifying-reth-to-build-the-fastest-transaction-network-on-bsc-and-polygon>

#![warn(unused_crate_dependencies)]

use chain_cfg::{boot_nodes, head, polygon_chain_spec};
use reth_discv4::Discv4ConfigBuilder;
use reth_ethereum::network::{
    api::events::SessionInfo, config::NetworkMode, NetworkConfig, NetworkEvent,
    NetworkEventListenerProvider, NetworkManager,
};
use reth_tracing::{
    tracing::info, tracing_subscriber::filter::LevelFilter, LayerInfo, LogFormat, RethTracer,
    Tracer,
};
use secp256k1::{rand, SecretKey};
use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio_stream::StreamExt;

pub mod chain_cfg;

#[tokio::main]
async fn main() {
    // The ECDSA private key used to create our enode identifier.
    let secret_key = SecretKey::new(&mut rand::thread_rng());

    let _ = RethTracer::new()
        .with_stdout(LayerInfo::new(
            LogFormat::Terminal,
            LevelFilter::INFO.to_string(),
            "".to_string(),
            Some("always".to_string()),
        ))
        .init();

    // The local address we want to bind to
    let local_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 30303);

    // The network configuration
    let net_cfg = NetworkConfig::builder(secret_key)
        .set_head(head())
        .network_mode(NetworkMode::Work)
        .listener_addr(local_addr)
        .build_with_noop_provider(polygon_chain_spec());

    // Set Discv4 lookup interval to 1 second
    let mut discv4_cfg = Discv4ConfigBuilder::default();
    let interval = Duration::from_secs(1);
    discv4_cfg.add_boot_nodes(boot_nodes()).lookup_interval(interval);
    let net_cfg = net_cfg.set_discovery_v4(discv4_cfg.build());

    let net_manager = NetworkManager::eth(net_cfg).await.unwrap();

    // The network handle is our entrypoint into the network.
    let net_handle = net_manager.handle();
    let mut events = net_handle.event_listener();

    // NetworkManager is a long running task, let's spawn it
    tokio::spawn(net_manager);
    info!("Looking for Polygon peers...");

    while let Some(evt) = events.next().await {
        // For the sake of the example we only print the session established event
        // with the chain specific details
        if let NetworkEvent::ActivePeerSession { info, .. } = evt {
            let SessionInfo { status, client_version, .. } = info;
            let chain = status.chain;
            info!(?chain, ?client_version, "Session established with a new peer.");
        }
        // More events here
    }
    // We will be disconnected from peers since we are not able to answer to network requests
}
