use super::ProviderActions;
use crate::commons::blockchain;
use crate::conf::{self, Endpoint, EndpointActions};
use crate::requests::client::ReqwestClient;
use crate::requests::rpc::{
    JsonRpcParams, JsonRpcReq, JsonRpcReqBody, JsonRpcResponse, JSON_RPC_VER,
};
use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize};
#[derive(Debug, Clone)]
pub struct EthereumNode {
    pub endpoint: conf::Endpoint,
}
#[async_trait]
impl ProviderActions for EthereumNode {
    async fn parse_top_blocks(
        &mut self,
        n_block: u32,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>> {
        if !self.endpoint.available() {
            return Err("Error: Endpoint not available".into());
        }
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(None);
        let head = self.get_block_by_number(None, false).await?.pop().unwrap();
        let mut block_numbers = Vec::new();
        for i in 0..n_block {
            block_numbers.push(head.number - i as u64);
        }
        let blocks = self
            .get_block_by_number(Some(&block_numbers), false)
            .await?;
        for block in blocks {
            blockchain.add_block(blockchain::Block {
                hash: block.hash,
                height: block.number,
                time: block.timestamp,
                txs: block.transactions.len() as u64,
            });
        }
        blockchain.sort();
        self.endpoint.set_last_request();
        Ok(blockchain)
    }
}

impl EthereumNode {
    pub fn new(options: conf::EndpointOptions, network: conf::Network) -> EthereumNode {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: Some(ReqwestClient::new(options)),
            network: network,
            last_request: 0,
        };
        EthereumNode { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, net: conf::Network) -> Self {
        EthereumNode {
            endpoint: conf::Endpoint::test_new(url, net),
        }
    }
    pub async fn get_block_by_number(
        &mut self,
        block_numbers: Option<&Vec<u64>>,
        txs: bool,
    ) -> Result<Vec<Block>, Box<dyn std::error::Error + Send + Sync>> {
        let req = match block_numbers {
            Some(block_numbers) => {
                let mut batch = Vec::new();
                block_numbers.into_iter().for_each(|block_number| {
                    let body = JsonRpcReq {
                        jsonrpc: JSON_RPC_VER.to_string(),
                        method: "eth_getBlockByNumber".to_string(),
                        params: vec![
                            JsonRpcParams::String(format!("0x{:x}", block_number)),
                            JsonRpcParams::Bool(txs),
                        ],
                        id: 1,
                    };
                    batch.push(body);
                });
                JsonRpcReqBody::Batch(batch)
            }
            None => {
                let body = JsonRpcReq {
                    jsonrpc: JSON_RPC_VER.to_string(),
                    method: "eth_getBlockByNumber".to_string(),
                    params: vec![
                        JsonRpcParams::String("latest".to_string()),
                        JsonRpcParams::Bool(txs),
                    ],
                    id: 1,
                };
                JsonRpcReqBody::Single(body)
            }
        };
        let reqwest = self.endpoint.reqwest.as_ref().unwrap();
        let res = reqwest
            .rpc(
                &req,
                &conf::Protocol::Ethereum.to_string(),
                &self.endpoint.network.to_string(),
            )
            .await;
        match res {
            Ok(res) => match req {
                JsonRpcReqBody::Single(_) => {
                    let rpc_res: JsonRpcResponse<Block> = serde_json::from_str(&res)?;
                    Ok(vec![rpc_res.result.unwrap()])
                }
                JsonRpcReqBody::Batch(_) => {
                    let rpc_res: Vec<JsonRpcResponse<Block>> = serde_json::from_str(&res)?;
                    let contain_err = rpc_res.iter().any(|r| {
                        if r.error.is_some() || r.result.is_none() {
                            return true;
                        };
                        false
                    });
                    if contain_err {
                        error!(
                            "Error in batch response: {:?}",
                            rpc_res
                                .iter()
                                .filter(|r| r.error.is_some())
                                .collect::<Vec<_>>()
                        );
                        return Err("Error in batch response".into());
                    }
                    let res = rpc_res
                        .into_iter()
                        .map(|r| {
                            trace!("batch block: {:?}", r);
                            // catch case where result is empty, if so, return Err

                            r.result.unwrap()
                        })
                        .collect();
                    Ok(res)
                }
            },
            Err(err) => {
                return Err(err);
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Block {
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    #[serde(rename = "baseFeePerGas")]
    pub base_fee_per_gas: u64,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub difficulty: u64,
    #[serde(rename = "extraData")]
    pub extra_data: String,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    #[serde(rename = "gasLimit")]
    pub gas_limit: u64,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    #[serde(rename = "gasUsed")]
    pub gas_used: u64,
    pub hash: String,
    #[serde(rename = "logsBloom")]
    pub logs_bloom: String,
    pub miner: String,
    #[serde(rename = "mixHash")]
    pub mix_hash: String,
    pub nonce: String,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub number: u64,
    #[serde(rename = "parentHash")]
    pub parent_hash: String,
    #[serde(rename = "receiptsRoot")]
    pub receipts_root: String,
    #[serde(rename = "sha3Uncles")]
    pub sha3_uncles: String,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub size: u64,
    #[serde(rename = "stateRoot")]
    pub state_root: String,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub timestamp: u64,
    #[serde(deserialize_with = "deserialize_from_hex_to_u128")]
    #[serde(rename = "totalDifficulty")]
    pub total_difficulty: u128,
    #[serde(rename = "transactionsRoot")]
    pub transactions_root: String,
    pub uncles: Vec<String>,
    pub transactions: Vec<String>,
}

//FIXME: Should be merge in same function with deserialize_from_hex_to_u128
pub fn deserialize_from_hex_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let hex_str = s.trim_start_matches("0x");
    let z = u64::from_str_radix(hex_str, 16);
    Ok(z.unwrap())
}
// FIXME: Should be merge in same function with deserialize_from_hex_to_u64
pub fn deserialize_from_hex_to_u128<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let hex_str = s.trim_start_matches("0x");
    let z = u128::from_str_radix(hex_str, 16);
    Ok(z.unwrap())
}

#[cfg(test)]
mod test {

    extern crate env_logger;
    use super::*;
    use crate::tests;
    use std::env;
    #[tokio::test]
    async fn test_get_latest_block_by_number() {
        tests::setup();
        let mut ethereum_node = EthereumNode::test_new(
            &env::var("ETHEREUM_NODE_URL").unwrap(),
            conf::Network::Mainnet,
        );
        let block = ethereum_node
            .get_block_by_number(None, false)
            .await
            .unwrap();
        assert_eq!(block.len(), 1);
    }

    #[tokio::test]
    async fn test_get_multiple_block_by_number() {
        tests::setup();
        let block_len = 5;
        let mut ethereum_node = EthereumNode::test_new(
            &env::var("ETHEREUM_NODE_URL").unwrap(),
            conf::Network::Mainnet,
        );
        let block = ethereum_node
            .get_block_by_number(None, false)
            .await
            .unwrap();
        let head_n = block[0].number.clone();
        // create list of last 5 blocks from head
        let mut block_numbers = Vec::new();
        for i in 0..block_len {
            block_numbers.push(head_n - i);
        }
        let blocks = ethereum_node
            .get_block_by_number(Some(&block_numbers), false)
            .await
            .unwrap();

        for i in block_numbers {
            assert_eq!(
                blocks.iter().any(|b| b.number == i),
                true,
                "Requested Block number {} not found",
                i
            );
        }
        assert_eq!(
            blocks.len(),
            block_len as usize,
            "Block length {} ,expected {}",
            blocks.len(),
            block_len
        );
    }
    #[tokio::test]
    async fn test_parse_top_blocks() {
        tests::setup();
        let mut ethereum_node = EthereumNode::test_new(
            &env::var("ETHEREUM_NODE_URL").unwrap(),
            conf::Network::Mainnet,
        );
        let res = ethereum_node.parse_top_blocks(5).await.unwrap();
        assert_eq!(res.blocks.len(), 5);
    }
}