use crate::commons::blockchain::{self};
use async_trait::async_trait;
pub mod bitcoin_node;
pub mod blockcypher;
pub mod blockstream;
pub mod ethereum_node;
pub mod polkadot_node;
pub mod subscan;
pub mod tezos_node;
pub mod tzkt;
pub mod tzstats;

#[async_trait]
pub trait ProviderActions: Send {
    // parse_top_blocks return basic task to parse top blocks
    async fn parse_top_blocks(
        &mut self,
        n_block: u32, // number of block to look ahead
        previous_head: Option<String>,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>>;
}
