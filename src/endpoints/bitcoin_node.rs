use super::ProviderActions;
use crate::commons::blockchain::{self};
use crate::conf::{self, Endpoint, Network, Protocol};
use crate::prom::registry::set_blockchain_height_endpoint;

use crate::requests::client::ReqwestClient;
use crate::requests::rpc::{
    JsonRpcParams, JsonRpcReq, JsonRpcReqBody, JsonRpcResponse, JSON_RPC_VER,
};
use async_trait::async_trait;

use serde::{Deserialize, Serialize};
#[derive(Serialize, Debug, Clone)]
pub struct BitcoinNode {
    pub endpoint: conf::Endpoint,
}

impl BitcoinNode {
    pub fn new(
        options: conf::EndpointOptions,
        protocol: Protocol,
        network: Network,
    ) -> BitcoinNode {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
            protocol,
            network,
            last_request: 0,
        };
        BitcoinNode { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: Network) -> Self {
        BitcoinNode {
            endpoint: conf::Endpoint::test_new(url, proto, net, None, None),
        }
    }
}
#[async_trait]
impl ProviderActions for BitcoinNode {
    /* Bitcoin Rpc work like this:
    1. Get the best block hash
    2. Get the block
    3. Get the previous block hash
    4. Repeat 2 and 3 until the number of blocks is reached
    */
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
        let bbh_res = self.get_best_block_hash().await;
        let best_block_hash = match bbh_res {
            Ok(hash) => hash,
            Err(e) => {
                trace!("Error: {}", e);
                return Err(e);
            }
        };

        /*
         * If the previous head is the same as the best block hash, we don't need to do anything
         */
        if let Some(prev_head) = previous_head {
            trace!("compare {} and {}", prev_head, best_block_hash);
            if prev_head == best_block_hash {
                debug!("No new block (head: {}), skip task", best_block_hash);
                return Err("No new block".into());
            }
        }

        let mut prev_block_hash = best_block_hash;
        for _ in 0..n_block {
            let res = self.get_block(prev_block_hash.as_str()).await;
            match res {
                Ok(block) => {
                    prev_block_hash = block.previousblockhash;
                    blockchain.add_block(blockchain::Block {
                        hash: block.hash,
                        height: block.height,
                        time: block.time,
                        txs: block.tx.len() as u64,
                    });
                }
                Err(e) => {
                    trace!("Error: {}", e);
                    break;
                }
            }
        }
        if blockchain.blocks.len() < n_block as usize {
            return Err("Error: build blockchain is less than n_block".into());
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

impl BitcoinNode {
    pub async fn get_best_block_hash(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        trace!("Get best block hash for {}", self.endpoint.url);
        let body = JsonRpcReqBody::Single(JsonRpcReq {
            jsonrpc: JSON_RPC_VER.to_string(),
            id: 1,
            method: "getbestblockhash".to_string(),
            params: vec![],
        });
        let client = &mut self.endpoint.reqwest;
        let res: JsonRpcResponse<String> = client
            .rpc(&body, &self.endpoint.protocol, &self.endpoint.network)
            .await?;
        Ok(res.result.unwrap())
    }
    pub async fn get_block(
        &mut self,
        hash: &str,
    ) -> Result<Getblock, Box<dyn std::error::Error + Send + Sync>> {
        trace!("Get block for {}", self.endpoint.url);
        let body = JsonRpcReqBody::Single(JsonRpcReq {
            jsonrpc: JSON_RPC_VER.to_string(),
            id: 1,
            method: "getblock".to_string(),
            params: vec![
                JsonRpcParams::String(hash.to_string()),
                JsonRpcParams::Number(1),
            ],
        });
        let client = &mut self.endpoint.reqwest;
        let res: JsonRpcResponse<Getblock> = client
            .rpc(&body, &self.endpoint.protocol, &self.endpoint.network)
            .await?;
        Ok(res.result.unwrap())
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Getblockchaininfo {
    pub chain: String,
    pub blocks: i64,
    pub headers: i64,
    pub bestblockhash: String,
    pub difficulty: f64,
    pub mediantime: i64,
    pub pruned: bool,
    pub bip9_softforks: BIP9Softforks,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct Getblock {
    pub hash: String,
    pub confirmations: i64,
    pub strippedsize: i64,
    pub size: i64,
    pub weight: i64,
    pub height: u64,
    pub version: i64,
    #[serde(rename = "versionHex")]
    pub versionhex: String,
    pub merkleroot: String,
    pub tx: Vec<String>,
    pub time: u64,
    pub nonce: i64,
    pub bits: String,
    pub difficulty: f64,
    pub previousblockhash: String,
    pub nextblockhash: Option<String>,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct BIP9Softforks {
    pub csv: BIP9,
    pub dummy: BIP9,
    #[serde(rename = "dummy-min-activation")]
    pub dummy_min_activation: BIP9,
    pub segwit: BIP9,
    pub taproot: BIP9,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct BIP9 {
    pub status: String,
    pub bit: i64,
    pub start_time: i64,
    pub timeout: i64,
    pub since: i64,
    pub min_activation_height: i64,
}

#[cfg(test)]

mod test {
    extern crate env_logger;
    use super::*;
    use crate::{conf::Network, tests};
    use std::env;

    #[tokio::test]
    async fn bitcoin_node_get_best_block_hash() {
        tests::setup();
        let url = env::var("BITCOIN_NODE_URL").unwrap();
        let mut bitcoin_node =
            BitcoinNode::test_new(url.as_str(), Protocol::Bitcoin, Network::Mainnet);
        let res = bitcoin_node.get_best_block_hash().await;
        assert!(
            res.is_ok(),
            "get_best_block_hash returned error: {}, expected OK",
            res.err().unwrap()
        );
        assert!(res.unwrap().len() > 0)
    }
    #[tokio::test]
    async fn bitcoin_node_get_block() {
        tests::setup();
        let url = env::var("BITCOIN_NODE_URL").unwrap();
        let mut bitcoin_node =
            BitcoinNode::test_new(url.as_str(), Protocol::Bitcoin, Network::Mainnet);
        let res = bitcoin_node
            .get_block(&"00000000000000000005bdd33e8c4ac8b3b1754f72416b9cb88ce278ea25f6ce")
            .await;
        assert!(
            res.is_ok(),
            "get_block returned error: {}, expected OK",
            res.err().unwrap()
        );
        let res = res.unwrap();
        assert_eq!(
            &res.hash, "00000000000000000005bdd33e8c4ac8b3b1754f72416b9cb88ce278ea25f6ce",
            "get_block returned wrong hash {}, expected {}",
            &res.hash, "00000000000000000005bdd33e8c4ac8b3b1754f72416b9cb88ce278ea25f6ce"
        );
    }
}
