use async_trait::async_trait;
use chrono::DateTime;
use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    commons::blockchain::{self, Block},
    conf::{self, Endpoint, EndpointActions, Protocol},
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
        nb_blocks: u32,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>> {
        if !self.endpoint.available() {
            return Err("Error: Endpoint not available".into());
        }
        let height = self.get_chain_height().await?;
        let blocks = self.get_blocks_from_height(height, nb_blocks).await?;
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(Some(blocks));
        blockchain.sort();
        self.endpoint.set_last_request();

        Ok(blockchain)
    }
}

impl Blockcypher {
    pub fn new(options: conf::EndpointOptions, network: conf::Network) -> Blockcypher {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: Some(ReqwestClient::new(options)),
            network: network,
            last_request: 0,
        };
        Blockcypher { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, net: crate::conf::Network) -> Self {
        Blockcypher {
            endpoint: conf::Endpoint::test_new(url, net),
        }
    }
    async fn get_chain_height(&mut self) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        trace!("Get head blockcypher");
        let url = format!("{}/v1/btc/main", self.endpoint.url);
        let res = self.run_request::<HeightResponse>(&url).await;
        match res {
            Ok(res) => Ok(res.height),
            Err(e) => {
                error!("Error while getting chain height: {}", e);
                Err(e)
            }
        }
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
            let url = format!("{}/v1/btc/main/blocks/{}", self.endpoint.url, height - i);
            let res = self.run_request::<BlockResponse>(&url).await;
            match res {
                Ok(res) => {
                    let datetime = DateTime::parse_from_rfc3339(&res.time).unwrap();
                    let timestamp = datetime.timestamp();
                    blocks.push(Block {
                        hash: res.hash,
                        height: res.height as u64,
                        time: timestamp as u64,
                        txs: res.n_tx as u64,
                    });
                }
                Err(e) => {
                    error!("Error while getting blocks from height: {}", e);
                    return Err(e);
                }
            }
        }
        Ok(blocks)
    }
    async fn run_request<T: DeserializeOwned>(
        &mut self,
        url: &str,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        trace!("Run blockcypher request: {}", url);
        let client = self.endpoint.reqwest.clone().unwrap();
        let res = client
            .get(
                &url,
                &Protocol::Bitcoin.to_string(),
                &self.endpoint.network.to_string(),
            )
            .await;
        trace!("Blockcypher response: {:?}", res);
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                error!("Blockcypher error: {}", e);
                return Err(e);
            }
        };
        let deserialize = serde_json::from_str::<T>(&res);
        match deserialize {
            Ok(deserialize) => Ok(deserialize),
            Err(e) => {
                error!("Blockcypher deserialize error: {}", e);
                return Err(e.into());
            }
        }
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
    use super::*;
    use crate::tests;

    #[tokio::test]
    async fn test_get_chain_height() {
        tests::setup();
        let mut blockcypher =
            Blockcypher::test_new("https://api.blockcypher.com", crate::conf::Network::Mainnet);
        let res = blockcypher.get_chain_height().await.unwrap();
        assert!(res > 0);
    }

    #[tokio::test]
    async fn test_get_blocks_from_height() {
        tests::setup();
        let n_block = 10;
        let height = 100;
        let mut blockcypher =
            Blockcypher::test_new("https://api.blockcypher.com", crate::conf::Network::Mainnet);
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
