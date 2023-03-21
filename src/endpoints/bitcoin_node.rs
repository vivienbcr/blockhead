use crate::commons::blockchain;
use crate::configuration::{self, EndpointOptions, NetworkName};
// use crate::configuration::Endpoint;
use crate::requests::client::{ReqwestClient};
use crate::requests::rpc::{JsonRpcBody, JsonRpcParams, JsonRpcResponse};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::Endpoint;

const JSON_RPC_VER: &str = "2.0";
#[derive(Deserialize, Serialize,Debug,Clone)]
pub struct BitcoinNode {
    pub url: String,
    pub options: Option<EndpointOptions>,
    #[serde(skip)]
    pub reqwest: Option<ReqwestClient>,
}
#[async_trait]
impl Endpoint for BitcoinNode {
    async fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let default_endpoint_opts = configuration::CONFIGURATION.get().unwrap().get_global_endpoint_config();
        let local_opts = self.options.clone().unwrap();
        let endpoint_opt = Some(EndpointOptions {
            url: Some(self.url.clone()),
            retry: local_opts.retry.or(default_endpoint_opts.retry),
            rate: local_opts.rate.or(default_endpoint_opts.rate),
            delay : local_opts.delay.or(default_endpoint_opts.delay)
        });
        self.reqwest = Some(ReqwestClient::new(endpoint_opt.clone().unwrap()));
        Ok(())
    }
    async fn parse_top_blocks(
        &self,
        network: &str,
        n_block: usize,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>> {
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(
            &configuration::ProtocolName::Bitcoin.to_string(),
            network,
        );
        let bbh_res = self.get_best_block_hash().await;
        let best_block_hash = match bbh_res {
            Ok(hash) => hash,
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
        reqwest_config: EndpointOptions,
        network: String,
    ) -> BitcoinNode {
        BitcoinNode {
            url: "http://localhost:8332".to_string(),
            options: None,
            reqwest: Some(ReqwestClient::new(reqwest_config))
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
            params: vec![
                JsonRpcParams::String(hash.to_string()),
                JsonRpcParams::Number(1),
            ],
        };
        self.run(&body).await
    }

    async fn run<T: DeserializeOwned>(
        &self,
        body: &JsonRpcBody,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let reqwest = self.reqwest.as_ref().unwrap();
        let res = reqwest
            .rpc(
                &body,
                &configuration::ProtocolName::Bitcoin.to_string(),
                &"FIXME NETWORK",
            )
            .await;
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
