use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    commons::blockchain,
    conf::{self, Endpoint, EndpointActions, Protocol},
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
        nb_blocks: u32,
        previous_head: Option<String>,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>> {
        if !self.endpoint.available() {
            return Err("Error: Endpoint not available".into());
        }
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(None);
        let tip = self.get_chain_tip().await?;
        if let Some(previous_head) = previous_head {
            if previous_head == tip.id {
                debug!(
                    "No new block (head: {} block with hash {}), skip task",
                    tip.height, tip.id
                );
                self.endpoint.set_last_request();
                return Err("No new block".into());
            }
        }

        let mut height = tip.height;

        let mut blocks = self.get_blocks_from_height(height).await?;
        while blocks.len() > 0 && blockchain.blocks.len() < nb_blocks as usize {
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
        self.endpoint.set_last_request();
        blockchain.sort();
        // remove blocks to return vec len = nb_blocks
        if blockchain.blocks.len() > nb_blocks as usize {
            blockchain.blocks.truncate(nb_blocks as usize);
        }
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
            reqwest: Some(ReqwestClient::new(options)),
            protocol,
            network,
            last_request: 0,
        };
        Blockstream { endpoint }
    }
    async fn get_blocks_from_height(
        &self,
        height: u64,
    ) -> Result<Vec<Block>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/blocks/{}", self.endpoint.url, height);
        let client = self.endpoint.reqwest.as_ref().unwrap();
        let res: Vec<Block> = client
            .get(
                &url,
                &conf::Protocol::Bitcoin.to_string(),
                &self.endpoint.network.to_string(),
            )
            .await?;
        Ok(res)
    }

    async fn get_chain_tip(&self) -> Result<Block, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/blocks/tip", self.endpoint.url);
        let client = self.endpoint.reqwest.as_ref().unwrap();
        let res: Vec<Block> = client
            .get(
                &url,
                &conf::Protocol::Bitcoin.to_string(),
                &self.endpoint.network.to_string(),
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
