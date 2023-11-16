use std::collections::HashMap;

use async_trait::async_trait;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use super::ProviderActions;
use crate::commons::blockchain;

use crate::conf::{self, Endpoint, Network, Protocol};
use crate::prom::registry::set_blockchain_height_endpoint;
use crate::requests::client::ReqwestClient;

#[derive(Serialize, Debug, Clone)]
pub struct TezosNode {
    pub endpoint: conf::Endpoint,
}
impl TezosNode {
    pub fn new(options: conf::EndpointOptions, protocol: Protocol, network: Network) -> TezosNode {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
            protocol,
            network,
            last_request: 0,
        };
        TezosNode { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: Network) -> Self {
        TezosNode {
            endpoint: conf::Endpoint::test_new(url, proto, net, None, None),
        }
    }
}
#[async_trait]
impl ProviderActions for TezosNode {
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
        let previous_head: String = previous_head.unwrap_or("".to_string());

        let head = self.get_block(None).await?;

        if previous_head == head.hash {
            debug!(
                "No new block (head: {} block with hash {}), skip task",
                head.header.level, head.hash
            );
            return Err("No new block".into());
        }

        let mut height = head.header.level;
        let mut blocks: Vec<blockchain::Block> = Vec::new();
        let head = head.to_block();
        blocks.push(head);

        let mut i = 1;
        while i < n_block {
            height -= 1;
            let res = self.get_block(Some(&height.to_string())).await?;
            let txs = *res.count_tx().get("transaction").unwrap_or(&0);
            let datetime = DateTime::parse_from_rfc3339(&res.header.timestamp).unwrap();
            let timestamp = datetime.timestamp();
            let b = blockchain::Block {
                hash: res.hash,
                height: res.header.level,
                time: timestamp as u64,
                txs,
            };
            blocks.push(b.clone());
            if b.hash == previous_head {
                debug!("Previous head found, stop parsing blocks");
                break;
            }
            i += 1;
        }
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
impl TezosNode {
    async fn get_block(
        &mut self,
        hash_or_height: Option<&str>,
    ) -> Result<TezosBlock, Box<dyn std::error::Error + Send + Sync>> {
        debug!(
            "GET /chains/main/blocks/{}",
            hash_or_height.unwrap_or("head")
        );
        let url = format!(
            "{}/chains/main/blocks/{}",
            self.endpoint.url,
            hash_or_height.unwrap_or("head")
        );
        let client = &mut self.endpoint.reqwest;
        let res: TezosBlock = client
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
}

pub type OpCounter = HashMap<String, u64>;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BlockHeader {
    pub context: String,
    pub fitness: Vec<String>,
    pub level: u64,
    pub liquidity_baking_toggle_vote: String,
    pub operations_hash: String,
    pub payload_hash: String,
    pub payload_round: u64,
    pub predecessor: String,
    pub proof_of_work_nonce: String,
    pub proto: u64,
    pub signature: String,
    pub timestamp: String,
    pub validation_pass: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Operation {
    pub branch: String,
    pub chain_id: String,
    pub contents: Vec<serde_json::Value>,
    pub hash: String,
    pub protocol: String,
    pub signature: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LevelInfo {
    pub cycle: u64,
    pub cycle_position: u64,
    pub expected_commitment: bool,
    pub level: u64,
    pub level_position: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Metadata {
    pub baker: String,
    pub baker_consensus_key: String,
    pub balance_updates: Vec<serde_json::Value>,
    pub deactivated: Vec<serde_json::Value>,
    pub consumed_milligas: String,
    pub implicit_operations_results: Vec<serde_json::Value>,
    pub level_info: LevelInfo,
    pub liquidity_baking_toggle_ema: i64,
    pub max_block_header_length: i64,
    pub max_operation_data_length: i64,
    pub max_operation_list_length: Vec<serde_json::Value>,
    pub max_operations_ttl: i64,
    pub next_protocol: String,
    pub nonce_hash: Option<String>,
    pub proposer: String,
    pub proposer_consensus_key: String,
    pub protocol: String,
    pub test_chain_status: serde_json::Value,
    pub voting_period_info: serde_json::Value,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TezosBlock {
    pub chain_id: String,
    pub hash: String,
    pub header: BlockHeader,
    pub metadata: Metadata,
    pub operations: Vec<Vec<Operation>>,
    pub protocol: String,
}

impl TezosBlock {
    pub fn to_block(&self) -> blockchain::Block {
        let txs = self.count_tx();
        let transactions_count = *txs.get("transaction").unwrap_or(&0);
        let datetime = DateTime::parse_from_rfc3339(&self.header.timestamp).unwrap();
        let timestamp = datetime.timestamp();
        blockchain::Block {
            hash: self.hash.clone(),
            height: self.header.level,
            time: timestamp as u64,
            txs: transactions_count,
        }
    }
    pub fn count_tx(&self) -> OpCounter {
        let mut op_count: OpCounter = HashMap::new();
        let mut fees_sum: u64 = 0;
        for op_scope in &self.operations {
            for op in op_scope {
                for content in op.contents.clone() {
                    let c = content.as_object();
                    if let Some(c) = c {
                        let kind = c.get("kind");
                        if let Some(kind) = kind {
                            let kind = kind.as_str().unwrap();
                            let counter = op_count.entry(kind.to_string()).or_insert(0);
                            *counter += 1;
                        }
                        let fees = c.get("fee");

                        if let Some(fees) = fees {
                            let fees = fees.as_str().unwrap();
                            let fees = fees.parse::<u64>().unwrap();
                            fees_sum += fees;
                        }
                    }
                }
            }
        }
        trace!("fees_sum: {}", fees_sum);
        trace!("op_count: {:?}", op_count);

        op_count
    }
}

#[cfg(test)]

mod tests {
    extern crate env_logger;
    use std::env;

    use super::*;
    use crate::tests;
    #[tokio::test]
    async fn tezos_get_block() {
        tests::setup();
        let url = env::var("TEZOS_NODE_URL").unwrap();
        let mut tezos_node = TezosNode::test_new(&url, Protocol::Tezos, Network::Mainnet);
        let r = tezos_node.get_block(None).await;
        assert!(r.is_ok());
        let block_head = r.unwrap();
        assert!(
            block_head.header.level > 0,
            "Block level should be greater than 0"
        );
    }
    #[tokio::test]
    async fn tezos_count_tx() {
        tests::setup();
        let url = env::var("TEZOS_NODE_URL").unwrap();
        let mut tezos_node = TezosNode::test_new(&url, Protocol::Tezos, Network::Mainnet);
        let r = tezos_node.get_block(None).await;

        assert!(r.is_ok());
    }
    #[tokio::test]
    async fn tezos_parse_top_blocks() {
        tests::setup();
        let url = env::var("TEZOS_NODE_URL").unwrap();
        let mut tezos_node = TezosNode::test_new(&url, Protocol::Tezos, Network::Mainnet);
        let r = tezos_node.parse_top_blocks(10, None).await;
        assert!(r.is_ok());
        let blockchain = r.unwrap();
        assert_eq!(
            blockchain.blocks.len(),
            10,
            "Block level should be equal to 10"
        );
        // Each block should be head - 1 , 2 , 3 , 4 , 5
        let mut i = 0;
        while i < 10 {
            assert_eq!(
                blockchain.blocks[i].height,
                blockchain.blocks[0].height - i as u64,
                "Block height should be equal to head - {}",
                i
            );
            i += 1;
        }
    }
}
