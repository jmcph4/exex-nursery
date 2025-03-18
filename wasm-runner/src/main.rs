use alloy_sol_types::sol;
use futures::{Future, TryStreamExt};
use reth::primitives::Log;
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use reth_tracing::tracing::info;

const BYTECODE_REGISTRY_ADDRESS: &str = "";

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

fn on_req(req: &ExecutionRequestEvent) -> eyre::Result<()> {
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
                new.blocks_and_receipts()
                    .map(|(_, receipt)| receipt)
                    .flat_map(|receipt| {
                        receipt.logs.iter().filter(|log| {
                            log.address == BYTECODE_REGISTRY_ADDRESS
                        })
                    })
                    .map(|log| ExecutionRequestEvent::decode_raw_log(log))
                    .for_each(|ev| on_req(ev));
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
}
