use crate::commons::blockchain;
use async_trait::async_trait;
pub mod bitcoin_node;
pub mod blockcypher;
pub mod blockstream;
pub mod ethereum_node;

#[async_trait]
pub trait ProviderActions {
    // parse_top_blocks return basic task to parse top blocks
    async fn parse_top_blocks(
        &mut self,
        n_block: u32,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>>;
}
