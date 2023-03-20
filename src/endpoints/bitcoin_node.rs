use crate::commons::blockchain;
// use crate::configuration::Endpoint;
use crate::requests::client::{ ReqwestClient, ReqwestConfig};
use crate::requests::rpc::{JsonRpcBody, JsonRpcResponse, JsonRpcParams};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

use super::Endpoint;
// create new const
const PROTOCOL: &str = "bitcoin";
const JSON_RPC_VER : &str = "2.0";
pub struct BitcoinNode {
    pub reqwest: ReqwestClient,
    pub last_block: i64,
    pub last_block_time: i64,
    pub last_block_hash: String,
    pub chain_param_block_time: i64,
    pub last_check: i64,
    pub network: String,
}
#[async_trait]
impl Endpoint for BitcoinNode {
    async fn parse_top_blocks(&self, n_block : usize) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>>{
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new("bitcoin", self.network.as_str());
        let bbh_res = self.get_best_block_hash().await;
        let best_block_hash = match bbh_res {
            Ok(hash) => {
                hash
            }
            Err(e) => {
                println!("Error: {}", e);
                return Err(e);
            }
        };
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
                    println!("Error: {}", e);
                    break;
                }
            }
        }
        if blockchain.blocks.len() < n_block {
            return Err("Error: build blockchain is less than n_block".into());
        }
        blockchain.finalize();
        Ok(blockchain)
    }
}

impl BitcoinNode {
    pub fn new(
        reqwest_config: ReqwestConfig,
        chain_param_block_time: i64,
        network: String,
    ) -> BitcoinNode {
        BitcoinNode {
            reqwest: ReqwestClient::new(reqwest_config),
            last_block: 0,
            last_block_time: 0,
            last_block_hash: "".to_string(),
            chain_param_block_time,
            last_check: 0,
            network,
        }
    }
    pub async fn get_blockchain_info(
        &self,
    ) -> Result<Getblockchaininfo, Box<dyn std::error::Error + Send + Sync>> {
        let body = JsonRpcBody {
            jsonrpc: JSON_RPC_VER.to_string(),
            id: 1,
            method: "getblockchaininfo".to_string(),
            params: vec![],
        };
        self.run(&body).await
    }
    pub async fn get_best_block_hash(
        &self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let body = JsonRpcBody {
            jsonrpc: JSON_RPC_VER.to_string(),
            id: 1,
            method: "getbestblockhash".to_string(),
            params: vec![],
        };
        self.run(&body).await
    }
    pub async fn get_block(
        &self,
        hash: &str,
    ) -> Result<Getblock, Box<dyn std::error::Error + Send + Sync>> {
        let body = JsonRpcBody {
            jsonrpc: JSON_RPC_VER.to_string(),
            id: 1,
            method: "getblock".to_string(),
            params: vec![JsonRpcParams::String(hash.to_string()),JsonRpcParams::Number(1)],
        };
        self.run(&body).await
    }

    async fn run<T: DeserializeOwned>(&self,  body : &JsonRpcBody) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let res = self.reqwest.rpc(&body, PROTOCOL, &self.network).await;
        if res.is_err() {
            return Err(res.err().unwrap());
        }
        let rpc_res: JsonRpcResponse<T> = serde_json::from_str(&res.unwrap())?;
        return Ok(rpc_res.result.unwrap());
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
