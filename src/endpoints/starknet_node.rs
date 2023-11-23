use async_trait::async_trait;

use serde::{Deserialize, Serialize};

use super::ProviderActions;
use crate::commons::blockchain;

use crate::conf::{self, Endpoint, Network, Protocol};
use crate::prom::registry::set_blockchain_height_endpoint;
use crate::requests::client::ReqwestClient;
use crate::requests::rpc::{
    JsonRpcParams, JsonRpcReq, JsonRpcReqBody, JsonRpcResponse, JSON_RPC_VER,
};

#[derive(Serialize, Debug, Clone)]
pub struct StarknetNode {
    pub endpoint: conf::Endpoint,
}
#[async_trait]
impl ProviderActions for StarknetNode {
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
        let head = self.get_head().await?;
        if let Some(previous_head) = previous_head {
            if previous_head == head.block_hash {
                debug!(
                    "No new block (head: {} block with hash {}), skip task",
                    head.block_number, head.block_hash
                );
                return Err("No new block".into());
            }
        }
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(None);
        // create a vector of block numbers head - n_block
        let mut block_numbers = Vec::new();
        for i in 0..n_block {
            block_numbers.push(head.block_number - i as u64);
        }
        let blocks = self.get_blocks_by_number(&block_numbers).await?;
        for block in blocks {
            blockchain.add_block(blockchain::Block {
                hash: block.block_hash,
                height: block.block_number,
                time: block.timestamp,
                txs: block.transactions.len() as u64,
            });
        }
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

impl StarknetNode {
    pub fn new(
        options: conf::EndpointOptions,
        protocol: Protocol,
        network: Network,
    ) -> StarknetNode {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
            protocol,
            network,
            last_request: 0,
        };
        StarknetNode { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: crate::conf::Network) -> Self {
        StarknetNode {
            endpoint: conf::Endpoint::test_new(url, proto, net, None, None),
        }
    }

    async fn get_head(
        &mut self,
    ) -> Result<StarnetBlockHashAndNumber, Box<dyn std::error::Error + Send + Sync>> {
        let body = JsonRpcReq {
            jsonrpc: JSON_RPC_VER.to_string(),
            method: "starknet_blockHashAndNumber".to_string(),
            params: vec![],
            id: 1,
        };
        let req = JsonRpcReqBody::Single(body);
        let client = &mut self.endpoint.reqwest;
        let res: JsonRpcResponse<StarnetBlockHashAndNumber> = client
            .rpc(&req, &self.endpoint.protocol, &self.endpoint.network)
            .await?;
        trace!("head: {:?}", res);
        Ok(res.result.unwrap())
    }

    async fn get_blocks_by_number(
        &mut self,
        blocks: &[u64],
    ) -> Result<Vec<StarknetBlock>, Box<dyn std::error::Error + Send + Sync>> {
        let mut batch = Vec::new();
        let mut i = 0;
        blocks.iter().for_each(|block| {
            let body = JsonRpcReq {
                jsonrpc: JSON_RPC_VER.to_string(),
                method: "starknet_getBlockWithTxs".to_string(),
                params: vec![JsonRpcParams::SerdeValue(serde_json::json!({
                    "block_number": block
                }))],
                id: i,
            };
            batch.push(body);
            i += 1;
        });
        let req = JsonRpcReqBody::Batch(batch);
        let client = &mut self.endpoint.reqwest;
        let res: Vec<JsonRpcResponse<StarknetBlock>> = client
            .rpc(&req, &self.endpoint.protocol, &self.endpoint.network)
            .await?;
        let contain_err = res.iter().any(|r| {
            if r.error.is_some() || r.result.is_none() {
                return true;
            };
            false
        });
        if contain_err {
            error!(
                "Error in batch response: {:?}",
                res.iter().filter(|r| r.error.is_some()).collect::<Vec<_>>()
            );
            return Err("Error in batch response".into());
        }
        let res = res
            .into_iter()
            .map(|r| {
                trace!("batch block: {:?}", r);
                r.result.unwrap()
            })
            .collect();
        Ok(res)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct StarknetBlock {
    pub status: String,
    pub block_hash: String,
    pub parent_hash: String,
    pub block_number: u64,
    pub new_root: String,
    pub timestamp: u64,
    pub sequencer_address: String,
    pub transactions: Vec<serde_json::Value>,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
struct StarnetBlockHashAndNumber {
    pub block_hash: String,
    pub block_number: u64,
}

#[cfg(test)]

mod tests {
    extern crate env_logger;
    use std::env;

    use super::*;
    use crate::tests;

    #[tokio::test]
    async fn starknet_get_head() {
        tests::setup();
        let mut starknet_node = StarknetNode::test_new(
            &env::var("STARKNET_NODE_URL").unwrap(),
            Protocol::Starknet,
            Network::Mainnet,
        );
        let head = starknet_node.get_head().await;
        assert!(head.is_ok());
        let head = head.unwrap();
        assert!(head.block_number > 0);
    }

    #[tokio::test]
    async fn starknet_get_blocks_by_number() {
        tests::setup();
        let mut starknet_node = StarknetNode::test_new(
            &env::var("STARKNET_NODE_URL").unwrap(),
            Protocol::Starknet,
            Network::Mainnet,
        );
        let head = starknet_node.get_head().await.unwrap();
        let mut block_numbers = Vec::new();
        for i in 0..10 {
            block_numbers.push(head.block_number - i as u64);
        }
        let blocks = starknet_node.get_blocks_by_number(&block_numbers).await;
        assert!(blocks.is_ok());
        let blocks = blocks.unwrap();
        assert_eq!(blocks.len(), 10);
    }
}
