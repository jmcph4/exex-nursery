use clap::Parser;
use reth_node_ethereum::EthereumNode;

use cli::{DEFAULT_RETH_ARGS, DEFAULT_TESTNET, Opts};
use exex::exex_init;

mod cli;
mod exex;
mod wasm;

fn main() -> eyre::Result<()> {
    let opts = Opts::parse();

    let reth_args = if opts.dev {
        let mut xs = DEFAULT_RETH_ARGS.to_vec();
        xs.extend_from_slice(&["--chain", DEFAULT_TESTNET]);
        xs
    } else {
        DEFAULT_RETH_ARGS.to_vec()
    };

    reth::cli::Cli::try_parse_args_from(reth_args)?.run(
        |builder, _| async move {
            let handle = builder
                .node(EthereumNode::default())
                .install_exex("WASM Runtime Engine", exex_init)
                .launch()
                .await?;

            handle.wait_for_node_exit().await
        },
    )
}
