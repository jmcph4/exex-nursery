use std::str::FromStr;

use alloy_consensus::TxReceipt;
use alloy_primitives::{Address, Log};
use alloy_sol_types::sol;
use futures::{Future, TryStreamExt};
use reth_exex::{ExExContext, ExExEvent, ExExNotification};
use reth_node_api::FullNodeComponents;
use reth_tracing::tracing::info;

use crate::{cli::DEFAULT_BYTECODE_REGISTRY_ADDRESS, wasm::on_req};

sol!(BytecodeRegistryContract, "registry_abi.json");

pub struct ExecutionRequestEvent {
    pub code: Vec<u8>,
}

impl ExecutionRequestEvent {
    pub fn decode_raw_log(log: &Log) -> eyre::Result<Self> {
        Ok(Self {
            code: log.data.data.to_vec(),
        })
    }
}

pub async fn exex_init<Node: FullNodeComponents>(
    ctx: ExExContext<Node>,
) -> eyre::Result<impl Future<Output = eyre::Result<()>>> {
    Ok(exex(ctx))
}

pub async fn exex<Node: FullNodeComponents>(
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
                                /* SAFETY(jmcph4): we control this constant */
                                log.address
                                    == Address::from_str(
                                        DEFAULT_BYTECODE_REGISTRY_ADDRESS,
                                    )
                                    .unwrap()
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

#[cfg(test)]
mod tests {
    use reth_execution_types::{Chain, ExecutionOutcome};
    use reth_exex_test_utils::{PollOnce, test_exex_context};
    use std::pin::pin;

    #[tokio::test]
    pub async fn test_exex() -> eyre::Result<()> {
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
