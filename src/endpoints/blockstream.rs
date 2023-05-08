use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    commons::blockchain,
    conf::{self, Endpoint, Protocol},
    prom::registry::set_blockchain_height_endpoint,
    requests::client::ReqwestClient,
};

use super::ProviderActions;

#[derive(Debug, Clone)]
pub struct Blockstream {
    pub endpoint: Endpoint,
}

#[async_trait]
impl ProviderActions for Blockstream {
    async fn parse_top_blocks(
        &mut self,
        n_block: u32,
        previous_head: Option<String>,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>> {
        trace!(
            "parse_top_blocks: n_block: {} previous_head: {:?}",
            n_block,
            previous_head
        );
        if !self.endpoint.reqwest.available() {
            return Err("Endpoint is not available".into());
        }
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(None);
        let tip = self.get_chain_tip().await?;
        if let Some(previous_head) = previous_head {
            if previous_head == tip.id {
                debug!(
                    "No new block (head: {} block with hash {}), skip task",
                    tip.height, tip.id
                );
                return Err("No new block".into());
            }
        }

        let mut height = tip.height;

        let mut blocks = self.get_blocks_from_height(height).await?;
        while blocks.len() > 0 && blockchain.blocks.len() < n_block as usize {
            for block in blocks {
                blockchain.blocks.push(blockchain::Block {
                    hash: block.id,
                    height: block.height,
                    time: block.timestamp,
                    txs: block.tx_count,
                });
            }
            height = height - 10;
            blocks = self.get_blocks_from_height(height).await?;
        }
        blockchain.sort();
        // remove blocks to return vec len = n_block
        if blockchain.blocks.len() > n_block as usize {
            blockchain.blocks.truncate(n_block as usize);
        }
        set_blockchain_height_endpoint(
            &self.endpoint.url,
            &self.endpoint.protocol,
            &self.endpoint.network,
            blockchain.height,
        );
        Ok(blockchain)
    }
}

impl Blockstream {
    pub fn new(
        options: conf::EndpointOptions,
        protocol: Protocol,
        network: conf::Network,
    ) -> Blockstream {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
            protocol,
            network,
            last_request: 0,
        };
        Blockstream { endpoint }
    }
    async fn get_blocks_from_height(
        &mut self,
        height: u64,
    ) -> Result<Vec<Block>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/blocks/{}", self.endpoint.url, height);
        let client = &mut self.endpoint.reqwest;
        let res: Vec<Block> = client
            .run_request(
                reqwest::Method::GET,
                None,
                &url,
                &self.endpoint.protocol,
                &self.endpoint.network,
            )
            .await?;
        Ok(res)
    }

    async fn get_chain_tip(&mut self) -> Result<Block, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/blocks/tip", self.endpoint.url);
        let client = &mut self.endpoint.reqwest;
        let res: Vec<Block> = client
            .run_request(
                reqwest::Method::GET,
                None,
                &url,
                &self.endpoint.protocol,
                &self.endpoint.network,
            )
            .await?;
        if res.len() == 0 {
            return Err("Error: tip not found".into());
        }
        Ok(res[0].clone())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Block {
    pub id: String,
    pub height: u64,
    pub version: u64,
    pub timestamp: u64,
    pub tx_count: u64,
    pub size: u64,
    pub weight: u64,
    pub merkle_root: String,
    pub previousblockhash: String,
    pub mediantime: u64,
    pub nonce: u64,
    pub bits: u64,
    pub difficulty: u64,
}
