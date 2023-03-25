pub mod bitcoin_node;

pub mod blockstream;
use crate::commons::blockchain;
use async_trait::async_trait;

#[async_trait]
pub trait Endpoint {
    // parse_top_blocks return basic task to parse top blocks
    async fn parse_top_blocks(&mut self,n_block : u32) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>>;
    // available return true if the endpoint last call is < last call + rate
    fn available(&self) -> bool;
    
}
