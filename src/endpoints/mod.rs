use crate::commons::blockchain;
use async_trait::async_trait;
pub mod bitcoin_node;
pub mod blockcypher;
pub mod blockstream;
pub mod ethereum_node;
use crate::conf;

#[async_trait]
pub trait ProviderActions: Send {
    // parse_top_blocks return basic task to parse top blocks
    async fn parse_top_blocks(
        &mut self,
        n_block: u32,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>>;
    //TODO: parse_top_blocks should fail if head not changed (avoid to parse all blocks)

    // fn new(endpoint: crate::conf2::EndpointOptions, network: conf2::Network) -> Self;
}
