use super::ProviderActions;
use crate::commons::blockchain::{self};
use crate::conf::{self, Endpoint, EndpointOptions, Network, Protocol};
use crate::prom::registry::set_blockchain_height_endpoint;
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
        trace!(
            "parse_top_blocks: n_block: {} previous_head: {:?}",
            n_block,
            previous_head
        );
        if !self.endpoint.reqwest.available() {
            return Err("Endpoint is not available".into());
        }
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

impl EthereumNode {
    pub fn new(options: EndpointOptions, protocol: Protocol, network: Network) -> EthereumNode {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
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
                let mut id = 1;
                block_numbers.iter().for_each(|block_number| {
                    let body = JsonRpcReq {
                        jsonrpc: JSON_RPC_VER.to_string(),
                        method: "eth_getBlockByNumber".to_string(),
                        params: vec![
                            JsonRpcParams::String(format!("0x{:x}", block_number)),
                            JsonRpcParams::Bool(txs),
                        ],
                        id,
                    };
                    id += 1;
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
        let client = &mut self.endpoint.reqwest;

        let res = match req {
            JsonRpcReqBody::Single(_) => {
                let rpc_res: JsonRpcResponse<EthBlock> = client
                    .rpc(&req, &self.endpoint.protocol, &self.endpoint.network)
                    .await?;
                Ok(vec![rpc_res.result.unwrap()])
            }
            JsonRpcReqBody::Batch(_) => {
                let rpc_res: Vec<JsonRpcResponse<EthBlock>> = client
                    .rpc(&req, &self.endpoint.protocol, &self.endpoint.network)
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
#[serde(rename_all = "camelCase")]
pub struct EthBlock {
    #[serde(default, deserialize_with = "deserialize_from_hex_to_u64")]
    pub base_fee_per_gas: u64,
    #[serde(deserialize_with = "deserialize_from_hex_to_u128")]
    pub difficulty: u128,
    pub extra_data: String,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub gas_limit: u64,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub gas_used: u64,
    pub hash: String,
    pub logs_bloom: String,
    pub miner: String,
    pub mix_hash: Option<String>, // Options to deal with Forks
    pub nonce: Option<String>,    // Options to deal with Forks
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub number: u64,
    pub parent_hash: String,
    pub receipts_root: String,
    pub sha3_uncles: String,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub size: u64,
    pub state_root: String,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    pub timestamp: u64,
    //TODO: Some Eth forks use totalDifficulty > u128, we need use big number crate to support it
    // while we don't need to use it now, so just use String
    pub total_difficulty: String,
    pub transactions_root: String,
    pub uncles: Vec<String>,
    pub transactions: Vec<String>,
}

#[cfg(test)]
mod test {

    extern crate env_logger;
    use serde_json::json;

    use super::*;
    use crate::tests;
    use std::env;
    #[tokio::test]
    async fn eth_partial_response() {
        let json = json!({"jsonrpc":"2.0","id":1,"result":{"difficulty":"0x1","extraData":"0x00000000000000000000000000000000000000000000000000000000000000001ac48accb9c19525b40a01afbb46a108ee376b3546003d5e3314f8afe665f08db182a44bfd8110340000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","gasLimit":"0x47b760","gasUsed":"0x0","hash":"0x2d45d3d4b88b9d33ef594775f6740c2cf2273dc5761c70b1f34f934e7cb21383","logsBloom":"0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000","miner":"0x0000000000000000000000000000000000000000","mixHash":"0x0000000000000000000000000000000000000000000000000000000000000000","nonce":"0x0000000000000000","number":"0x0","parentHash":"0x0000000000000000000000000000000000000000000000000000000000000000","receiptsRoot":"0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421","sha3Uncles":"0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347","size":"0x286","stateRoot":"0x8832ddf41138266d665a6836abee79a57f76d0f02371684986010c7eb3a3a64e","timestamp":"0x5f462f38","totalDifficulty":"0x1","transactions":[],"transactionsRoot":"0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421","uncles":[]}});
        let res: JsonRpcResponse<EthBlock> = serde_json::from_value(json).unwrap();
        assert_eq!(res.result.unwrap().number, 0);
    }

    #[tokio::test]
    async fn eth_node_get_latest_block_by_number() {
        tests::setup();
        let mut ethereum_node = EthereumNode::test_new(
            &env::var("ETHEREUM_NODE_URL").unwrap(),
            Protocol::Ethereum,
            String::from("mainnet"),
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
            String::from("mainnet"),
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
            String::from("mainnet"),
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
            String::from("mainnet"),
        );
        let res = ethereum_node.parse_top_blocks(5, None).await.unwrap();
        assert_eq!(res.blocks.len(), 5);
    }
}
