use alloy_consensus::TxReceipt;
use alloy_primitives::{Address, Log, address};
use alloy_sol_types::sol;
use eyre::eyre;
use futures::{Future, TryStreamExt};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use reth_tracing::tracing::info;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::{
    IoView, WasiCtx, WasiCtxBuilder, WasiView, preview1::WasiP1Ctx,
};

const BYTECODE_REGISTRY_ADDRESS: Address =
    address!("d8da6bf26964af9d7eed9e03e53415d37aa96045");

sol!(BytecodeRegistryContract, "registry_abi.json");

struct ExecutionRequestEvent {
    pub code: Vec<u8>,
}

impl ExecutionRequestEvent {
    fn decode_raw_log(log: &Log) -> eyre::Result<Self> {
        Ok(Self {
            code: log.data.data.to_vec(),
        })
    }
}

struct WasiRuntimeContext(pub WasiP1Ctx);

impl IoView for WasiRuntimeContext {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        todo!()
    }
}

impl WasiView for WasiRuntimeContext {
    fn ctx(&mut self) -> &mut WasiCtx {
        todo!()
    }
}

impl WasiRuntimeContext {
    pub fn new() -> Self {
        Self(WasiCtxBuilder::new().build_p1())
    }

    pub fn ctx_mut(&mut self) -> &mut WasiP1Ctx {
        &mut self.0
    }
}

fn on_req(req: &ExecutionRequestEvent) -> eyre::Result<()> {
    let engine = Engine::default();
    let mut store: Store<WasiRuntimeContext> =
        Store::new(&engine, WasiRuntimeContext::new());
    let module = Module::from_binary(&engine, &req.code)
        .map_err(|e| eyre!("WASM error: {e:?}"))?;
    let mut linker: Linker<WasiRuntimeContext> = Linker::new(&engine);
    wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |ctx| {
        ctx.ctx_mut()
    })
    .map_err(|e| eyre!("WASM error: {e:?}"))?;
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|e| eyre!("WASM error: {e:?}"))?;

    if let Ok(start) = instance.get_typed_func::<(), ()>(&mut store, "_start") {
        start
            .call(&mut store, ())
            .map_err(|e| eyre!("WASM Error: {e:?}"))?;
    }

    Ok(())
}

async fn exex_init<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
    Ok(exex(ctx))
}

async fn exex<Node: FullNodeComponents>(
    mut ctx: ExExContext<Node>,
) -> eyre::Result<()> {
    while let Some(notification) = ctx.notifications.try_next().await? {
        match &notification {
            ExExNotification::ChainCommitted { new } => {
                info!(committed_chain = ?new.range(), "Received commit");
                new.block_receipts_iter()
                    .flatten()
                    .flat_map(|receipt| {
                        receipt
                            .logs()
                            .iter()
                            .filter(|log| {
                                log.address == BYTECODE_REGISTRY_ADDRESS
                            })
                            .collect::<Vec<_>>()
                    })
                    .try_for_each(|log| {
                        on_req(&ExecutionRequestEvent::decode_raw_log(log)?)
                    })?;
            }
            ExExNotification::ChainReorged { old, new } => {
                info!(from_chain = ?old.range(), to_chain = ?new.range(), "Received reorg");
            }
            ExExNotification::ChainReverted { old } => {
                info!(reverted_chain = ?old.range(), "Received revert");
            }
        };

        if let Some(committed_chain) = notification.committed_chain() {
            ctx.events.send(ExExEvent::FinishedHeight(
                committed_chain.tip().num_hash(),
            ))?;
        }
    }

    Ok(())
}

fn main() -> eyre::Result<()> {
    reth::cli::Cli::try_parse_args_from(["reth", "node"])?.run(
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

#[cfg(test)]
mod tests {
    use super::*;

    use reth_execution_types::{Chain, ExecutionOutcome};
    use reth_exex_test_utils::{PollOnce, test_exex_context};
    use std::pin::pin;

    #[tokio::test]
    async fn test_exex() -> eyre::Result<()> {
        let (ctx, mut handle) = test_exex_context().await?;
        let head = ctx.head;
        handle
            .send_notification_chain_committed(Chain::from_block(
                handle.genesis.clone(),
                ExecutionOutcome::default(),
                None,
            ))
            .await?;
        let mut exex = pin!(super::exex_init(ctx).await?);
        handle.assert_events_empty();
        exex.poll_once().await?;
        handle.assert_event_finished_height((head.number, head.hash).into())?;

        Ok(())
    }

    #[test]
    fn test_wasm_code_execution_success() {
        let source = r#"
(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 8) "Hello, borker!\n")

  (func $main (export "_start")
    (i32.store (i32.const 0) (i32.const 8))
    (i32.store (i32.const 4) (i32.const 14))
    (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 0))
    drop
  )
)
            "#;
        let code = wat2wasm(&source).unwrap();
        let actual_res = on_req(&ExecutionRequestEvent { code });
        dbg!(&actual_res);
        assert!(actual_res.is_ok());
    }

    fn wat2wasm(source: &str) -> eyre::Result<Vec<u8>> {
        Ok(wat::parse_str(source)?)
    }
}
