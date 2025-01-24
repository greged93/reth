//! Scroll binary

#[global_allocator]
static ALLOC: reth_cli_util::allocator::Allocator = reth_cli_util::allocator::new_allocator();

#[cfg(all(feature = "scroll", not(feature = "optimism")))]
fn main() {
    use clap::Parser;
    use reth_scroll_cli::{Cli, ScrollChainSpecParser, ScrollRollupArgs};
    use reth_scroll_node::ScrollNode;
    use tracing::info;

    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = Cli::<ScrollChainSpecParser, ScrollRollupArgs>::parse()
        .run::<_, _, ScrollNode>(|builder, _| async move {
            info!(target: "reth::cli", "Launching node");
            let handle = builder.launch_node(ScrollNode).await?;
            handle.node_exit_future.await
        })
    {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

#[cfg(any(feature = "optimism", not(feature = "scroll")))]
fn main() {
    eprintln!("Scroll feature is not enabled");
    std::process::exit(1);
}
