pub mod bitcoin_node;
use crate::commons::blockchain;
use async_trait::async_trait;

#[async_trait]
pub trait Endpoint {
    async fn parse_top_blocks(&self, n_block : usize) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>>;
}
