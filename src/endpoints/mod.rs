pub mod bitcoin_node;

pub mod blockstream;
use crate::commons::blockchain;
use async_trait::async_trait;

#[async_trait]
pub trait Endpoint {
    async fn parse_top_blocks(&self, network: &str,n_block : usize) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>>;
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
