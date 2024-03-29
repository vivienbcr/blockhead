use async_trait::async_trait;
use chrono::DateTime;
use serde::Deserialize;

use crate::{
    commons::blockchain::{self, Block},
    conf::{Endpoint, EndpointOptions, Network, Protocol},
    prom::registry::set_blockchain_height_endpoint,
    requests::client::ReqwestClient,
};

use super::ProviderActions;

#[derive(Debug, Clone)]
pub struct Blockcypher {
    pub endpoint: Endpoint,
}

#[async_trait]
impl ProviderActions for Blockcypher {
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
        let chain_state = self.get_chain_height().await?;
        if let Some(previous_head) = previous_head {
            if previous_head == chain_state.hash {
                debug!(
                    "No new block (head: {} block with hash {}), skip task",
                    chain_state.height, chain_state.hash
                );
                return Err("No new block".into());
            }
        }
        let height = chain_state.height;
        let blocks = self.get_blocks_from_height(height, n_block).await?;
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(Some(blocks));
        blockchain.sort();
        set_blockchain_height_endpoint(
            &self.endpoint.url,
            &self.endpoint.reqwest.config.alias,
            &self.endpoint.protocol,
            &self.endpoint.network,
            blockchain.height,
        );
        Ok(blockchain)
    }
}

impl Blockcypher {
    pub fn new(options: EndpointOptions, protocol: Protocol, network: Network) -> Blockcypher {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
            protocol,
            network,
            last_request: 0,
        };
        Blockcypher { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: Network) -> Self {
        Blockcypher {
            endpoint: Endpoint::test_new(url, proto, net, None, None),
        }
    }
    async fn get_chain_height(
        &mut self,
    ) -> Result<HeightResponse, Box<dyn std::error::Error + Send + Sync>> {
        trace!("Get head blockcypher");
        let url = self.endpoint.url.to_string();
        let client = &mut self.endpoint.reqwest;
        let res: HeightResponse = client
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
    async fn get_blocks_from_height(
        &mut self,
        height: u32,
        n_block: u32,
    ) -> Result<Vec<Block>, Box<dyn std::error::Error + Send + Sync>> {
        // its possible to batch request blocks with blockcypher but somes response will come with rate limit error
        // just get blocks one by one
        let mut blocks = Vec::new();
        for i in 0..n_block {
            let url = format!("{}/blocks/{}", self.endpoint.url, height - i);
            let client = &mut self.endpoint.reqwest;
            let res: BlockResponse = client
                .run_request(
                    reqwest::Method::GET,
                    None,
                    &url,
                    &self.endpoint.protocol,
                    &self.endpoint.network,
                )
                .await?;

            let datetime = DateTime::parse_from_rfc3339(&res.time).unwrap();
            let timestamp = datetime.timestamp();
            blocks.push(Block {
                hash: res.hash,
                height: res.height as u64,
                time: timestamp as u64,
                txs: res.n_tx as u64,
            });
        }

        Ok(blocks)
    }
}
#[derive(Deserialize, Debug)]
pub struct HeightResponse {
    pub name: String,
    pub height: u32,
    pub hash: String,
    pub time: String,
    pub latest_url: String,
    pub previous_hash: String,
    pub previous_url: String,
    pub peer_count: u32,
    pub unconfirmed_count: u32,
    pub high_fee_per_kb: u32,
    pub medium_fee_per_kb: u32,
    pub low_fee_per_kb: u32,
    pub last_fork_height: u32,
    pub last_fork_hash: String,
}
#[derive(Deserialize, Debug)]
pub struct BlockResponse {
    pub hash: String,
    pub height: u32,
    pub chain: String,
    pub total: u64,
    pub fees: u32,
    pub size: u32,
    pub vsize: u32,
    pub ver: u32,
    pub time: String,
    pub received_time: String,
    pub relayed_by: String,
    pub bits: u32,
    pub nonce: u32,
    pub n_tx: u32,
    pub prev_block: String,
    pub mrkl_root: String,
    pub txids: Vec<String>,
    pub depth: u32,
    pub prev_block_url: String,
    pub tx_url: String,
}
#[cfg(test)]

// log all info and print to stdout
mod tests {
    extern crate env_logger;
    use std::env;

    use super::*;
    use crate::tests;

    #[tokio::test]
    async fn blockcypherget_chain_height() {
        tests::setup();
        let url = env::var("BLOCKCYPHER_URL").unwrap();
        let mut blockcypher =
            Blockcypher::test_new(&url, Protocol::Bitcoin, String::from("mainnet"));
        let chain_state = blockcypher.get_chain_height().await.unwrap();
        assert!(chain_state.height > 0);
    }

    #[tokio::test]
    async fn blockcypher_get_blocks_from_height() {
        tests::setup();
        let n_block = 5;
        let height = 100;
        let url = env::var("BLOCKCYPHER_URL").unwrap();
        let mut blockcypher =
            Blockcypher::test_new(&url, Protocol::Bitcoin, String::from("mainnet"));
        let res = blockcypher
            .get_blocks_from_height(height, n_block)
            .await
            .unwrap();
        assert_eq!(
            res.len(),
            n_block as usize,
            "get_blocks_from_height return {} expected {}",
            res.len(),
            n_block
        );
        // check if we have nblocks from height
        for i in 0..n_block {
            let idx = height - i;
            let x = res.iter().find(|&x| x.height == idx as u64);
            assert!(
                x.is_some(),
                "get_blocks_from_height not return execpted height {}",
                idx
            );
        }
    }
}
