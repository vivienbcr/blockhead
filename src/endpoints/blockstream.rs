use std::{
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    commons::blockchain,
    configuration::{self},
};

use super::Endpoint;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Blockstream {
    pub endpoint: configuration::Endpoint,
}

#[async_trait]
impl Endpoint for Blockstream {
    async fn parse_top_blocks(
        &mut self,
        nb_blocks: u32,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>> {
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(
            &configuration::ProtocolName::Bitcoin.to_string(),
            &self.endpoint.network.to_string(),
        );
        let mut height = self.get_chain_height().await?;
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
        self.set_last_request();
        blockchain.sort();
        // remove blocks to return vec len = nb_blocks
        if blockchain.blocks.len() > nb_blocks as usize {
            blockchain.blocks.truncate(nb_blocks as usize);
        }
        Ok(blockchain)
    }
    fn available(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        let diff = now - self.endpoint.last_request;
        if diff < self.endpoint.reqwest.clone().unwrap().config.rate.unwrap() as u64 {
            debug!("Rate limit reached for {} ({}s)", self.endpoint.url, diff);
            return false;
        }
        true
    }
}

impl Blockstream {
    pub fn new(endpoint: configuration::Endpoint) -> Blockstream {
        Blockstream { endpoint }
    }
    fn set_last_request(&mut self) {
        trace!(
            "Set last request for {} to {}",
            self.endpoint.url,
            self.endpoint.last_request
        );
        self.endpoint.last_request = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
    }
    // get_block return last 10 blocks
    async fn get_blocks_from_height(
        &self,
        height: u32,
    ) -> Result<Vec<Block>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/blocks/{}", self.endpoint.url, height);
        self.run(&url).await
    }

    async fn get_chain_height(&self) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/blocks/tip/height", self.endpoint.url);
        self.run(&url).await
    }

    async fn run<T: DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let reqwest = self.endpoint.reqwest.clone().unwrap();
        let res = reqwest
            .get(
                url,
                &configuration::ProtocolName::Bitcoin.to_string(),
                &self.endpoint.network.to_string(),
            )
            .await;
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                debug!("Error Blockstream: {}", e);
                return Err("Error: reqwest".into());
            }
        };
        let res = serde_json::from_str::<T>(&res);
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                debug!("Error Blockstream: deserialize json response {}", e);
                return Err("Error: serde_json".into());
            }
        };
        return Ok(res);
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
