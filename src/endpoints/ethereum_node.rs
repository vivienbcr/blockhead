use super::ProviderActions;
use crate::commons::blockchain::{self};
use crate::conf::{self, Endpoint, EndpointOptions, Network, Protocol};
use crate::requests::client::ReqwestClient;
use crate::requests::rpc::{
    JsonRpcParams, JsonRpcReq, JsonRpcReqBody, JsonRpcResponse, JSON_RPC_VER,
};
use crate::utils::{deserialize_from_hex_to_u128, deserialize_from_hex_to_u64};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone)]
pub struct EthereumNode {
    pub endpoint: conf::Endpoint,
}
#[async_trait]
impl ProviderActions for EthereumNode {
    async fn parse_top_blocks(
        &mut self,
        n_block: u32,
        previous_head: Option<String>,
    ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>> {
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(None);
        let head = self.get_block_by_number(None, false).await?.pop().unwrap();

        if let Some(previous_head) = previous_head {
            if previous_head == head.hash {
                debug!(
                    "No new block (head: {} block with hash {}), skip task",
                    head.number, head.hash
                );
                return Err("No new block".into());
            }
        }

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
        let reqwest = self.endpoint.reqwest.as_mut().unwrap();
        reqwest.set_last_request();
        Ok(blockchain)
    }
}

impl EthereumNode {
    pub fn new(options: EndpointOptions, protocol: Protocol, network: Network) -> EthereumNode {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: Some(ReqwestClient::new(options)),
            protocol,
            network,
            last_request: 0,
        };
        EthereumNode { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: Network) -> Self {
        EthereumNode {
            endpoint: conf::Endpoint::test_new(url, proto, net, None, None),
        }
    }
    pub async fn get_block_by_number(
        &mut self,
        block_numbers: Option<&Vec<u64>>,
        txs: bool,
    ) -> Result<Vec<EthBlock>, Box<dyn std::error::Error + Send + Sync>> {
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
        let reqwest = self.endpoint.reqwest.as_mut().unwrap();

        let res = match req {
            JsonRpcReqBody::Single(_) => {
                let rpc_res: JsonRpcResponse<EthBlock> = reqwest
                    .rpc(
                        &req,
                        &conf::Protocol::Ethereum.to_string(),
                        &self.endpoint.network.to_string(),
                    )
                    .await?;
                Ok(vec![rpc_res.result.unwrap()])
            }
            JsonRpcReqBody::Batch(_) => {
                let rpc_res: Vec<JsonRpcResponse<EthBlock>> = reqwest
                    .rpc(
                        &req,
                        &conf::Protocol::Ethereum.to_string(),
                        &self.endpoint.network.to_string(),
                    )
                    .await?;
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
                        r.result.unwrap()
                    })
                    .collect();
                Ok(res)
            }
        };
        res
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EthBlock {
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    #[serde(rename = "baseFeePerGas")]
    pub base_fee_per_gas: u64,
    #[serde(deserialize_with = "deserialize_from_hex_to_u128")]
    pub difficulty: u128,
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
    pub mix_hash: Option<String>, // Options to deal with Forks
    pub nonce: Option<String>, // Options to deal with Forks
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
    //TODO: Some Eth forks use totalDifficulty > u128, we need use big number crate to support it
    // while we don't need to use it now, so just use String
    #[serde(rename = "totalDifficulty")]
    pub total_difficulty: String,
    #[serde(rename = "transactionsRoot")]
    pub transactions_root: String,
    pub uncles: Vec<String>,
    pub transactions: Vec<String>,
}

#[cfg(test)]
mod test {

    extern crate env_logger;
    use super::*;
    use crate::tests;
    use std::env;
    #[tokio::test]
    async fn eth_node_get_latest_block_by_number() {
        tests::setup();
        let mut ethereum_node = EthereumNode::test_new(
            &env::var("ETHEREUM_NODE_URL").unwrap(),
            Protocol::Ethereum,
            Network::Mainnet,
        );
        let block = ethereum_node
            .get_block_by_number(None, false)
            .await
            .unwrap();
        assert_eq!(block.len(), 1);
    }

    #[tokio::test]
    async fn eth_node_get_multiple_block_by_number() {
        tests::setup();
        let block_len = 5;
        let mut ethereum_node = EthereumNode::test_new(
            &env::var("ETHEREUM_NODE_URL").unwrap(),
            Protocol::Ethereum,
            Network::Mainnet,
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
    async fn eth_node_parse_top_blocks() {
        tests::setup();
        let mut ethereum_node = EthereumNode::test_new(
            &env::var("ETHEREUM_NODE_URL").unwrap(),
            Protocol::Ethereum,
            Network::Mainnet,
        );
        let res = ethereum_node.parse_top_blocks(5, None).await.unwrap();
        assert_eq!(res.blocks.len(), 5);
    }
    #[tokio::test]
    async fn eth_node_fork_parse_top_blocks() {
        tests::setup();
        let mut ethereum_node = EthereumNode::test_new(
            &env::var("EWF_NODE_URL").unwrap(),
            Protocol::Ethereum,
            Network::Mainnet,
        );
        let res = ethereum_node.parse_top_blocks(5, None).await.unwrap();
        assert_eq!(res.blocks.len(), 5);
    }
}
