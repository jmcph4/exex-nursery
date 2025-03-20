use alloy_primitives::Address;
use clap::Parser;

pub const DEFAULT_BYTECODE_REGISTRY_ADDRESS: &str =
    "d8da6bf26964af9d7eed9e03e53415d37aa96045";
pub const DEFAULT_RETH_ARGS: [&str; 2] = ["reth", "node"];
pub const DEFAULT_TESTNET: &str = "hoodi";

/// Executes arbitrary WebAssembly programs from Ethereum
#[derive(Clone, Debug, Parser)]
#[clap(author, version, about)]
pub struct Opts {
    #[clap(short, long, action)]
    pub dev: bool,
    #[clap(default_value = DEFAULT_BYTECODE_REGISTRY_ADDRESS)]
    pub bytecode_registry: Address,
}
