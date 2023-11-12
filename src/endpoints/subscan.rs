use async_trait::async_trait;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::ProviderActions;
use crate::commons::blockchain;

use crate::conf::{self, Endpoint, Network, Protocol};
use crate::prom::registry::set_blockchain_height_endpoint;
use crate::requests::client::ReqwestClient;

#[derive(Serialize, Debug, Clone)]
pub struct Subscan {
    pub endpoint: conf::Endpoint,
}

impl Subscan {
    pub fn new(options: conf::EndpointOptions, protocol: Protocol, network: Network) -> Subscan {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
            protocol,
            network,
            last_request: 0,
        };
        Subscan { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: crate::conf::Network) -> Self {
        Subscan {
            endpoint: conf::Endpoint::test_new(url, proto, net, None, None),
        }
    }
}
#[async_trait]
impl ProviderActions for Subscan {
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
        let previous_head = previous_head.unwrap_or("".to_string());
        let block_head = self.get_finalized_head().await;
        let block_head = match block_head {
            Ok(block_head) => block_head,
            Err(e) => {
                debug!("Error while getting head: {}", e);
                return Err(e);
            }
        };
        trace!(
            "Head hash {} height {}",
            block_head.hash,
            block_head.block_num
        );
        /*
         * Get the head block to check if there is a new block
         */
        if block_head.hash == previous_head {
            debug!(
                "No new block (head: {} block with hash {}), skip task",
                block_head.block_num, block_head.hash
            );
            return Err("No new block".into());
        }
        let mut blockchain = blockchain::Blockchain::new(None);
        let res = self.get_finalized_blocks(n_block as u16, None).await?;

        for block in res {
            let b = block.to_block();
            blockchain.add_block(b);
        }
        blockchain.sort();

        set_blockchain_height_endpoint(
            &self.endpoint.url,
            &self.endpoint.protocol,
            &self.endpoint.network,
            blockchain.height,
        );
        Ok(blockchain)
    }
}
const PAGE_MAX_ROW: u16 = 100;

impl Subscan {
    async fn get_finalized_head(
        &mut self,
    ) -> Result<SubscanBlock, Box<dyn std::error::Error + Send + Sync>> {
        let client = &mut self.endpoint.reqwest;
        let url = format!("{}/api/v2/scan/blocks", self.endpoint.url);
        let body = json!({
            "row": 10,
            "page": 0
        });
        let res: SubscanBlocksRes = client
            .run_request(
                reqwest::Method::POST,
                Some(body),
                &url,
                &self.endpoint.protocol,
                &self.endpoint.network,
            )
            .await?;
        // get highest number of block with finalized = true
        let block = res
            .data
            .blocks
            .iter()
            .filter(|b| b.finalized)
            .max_by_key(|b| b.block_num);
        match block {
            Some(block) => Ok(block.clone()),
            None => Err("No head".into()),
        }
    }
    async fn get_block(
        &mut self,
        height: u64,
    ) -> Result<SubscanBlock, Box<dyn std::error::Error + Send + Sync>> {
        let client = &mut self.endpoint.reqwest;
        let url = format!("{}/api/v2/scan/block", self.endpoint.url);
        let body = json!({
            "block_num": height,
        });
        let res: SubscanBlock = client
            .run_request(
                reqwest::Method::POST,
                Some(body),
                &url,
                &self.endpoint.protocol,
                &self.endpoint.network,
            )
            .await?;
        Ok(res)
    }

    async fn get_finalized_blocks(
        &mut self,
        n_block: u16,
        from_hash: Option<String>,
    ) -> Result<Vec<SubscanBlock>, Box<dyn std::error::Error + Send + Sync>> {
        // let from_hash = from_hash.unwrap_or("".to_string());
        trace!(
            "get_finalized_blocks: n_block: {}, hash : {:?}",
            n_block,
            from_hash,
        );
        let client = &mut self.endpoint.reqwest;
        let query_len = if PAGE_MAX_ROW < n_block + 30 {
            PAGE_MAX_ROW
        } else {
            n_block + 30
        };
        let url = format!("{}/api/v2/scan/blocks", self.endpoint.url);
        let body = json!({
            "row": query_len,
            "page": 0
        });
        let res: SubscanBlocksRes = client
            .run_request(
                reqwest::Method::POST,
                Some(body),
                &url,
                &self.endpoint.protocol,
                &self.endpoint.network,
            )
            .await?;
        let mut finalized_blocks: Vec<SubscanBlock> = res
            .data
            .blocks
            .iter()
            .filter(|b| b.finalized)
            .cloned()
            .collect();
        finalized_blocks.sort_by(|a, b| b.block_num.cmp(&a.block_num));
        let mut blocks = vec![];
        // from hash is none, we take just last blocks finalized from the last block
        let mut found = from_hash.is_none();
        let from_hash = from_hash.unwrap_or("".to_string());
        // find the block with the same hash as the previous head
        let mut expected_block_num = None;
        for block in finalized_blocks {
            if found {
                // blocks.push(block.clone());
                // if blocks.len() >= n_block as usize {
                //     break;
                // }
                // continue;
                if let Some(num) = expected_block_num {
                    if block.block_num != num {
                        return Err(format!("Block number {} is missing", num).into());
                    }
                }
                blocks.push(block.clone());
                if blocks.len() >= n_block as usize {
                    break;
                }
                expected_block_num = Some(block.block_num - 1);
                continue;
            }
            if block.hash == from_hash {
                found = true;
                blocks.push(block.clone());
                continue;
            }
        }
        if !found {
            error!("Block {} not found", from_hash);
            return Err("Block not found".into());
        }

        // if we didn't find all requested blocks, get blocks one by one to complete list
        if blocks.len() < n_block as usize {
            let mut height = blocks.last().unwrap().block_num;
            while blocks.len() < n_block as usize {
                height -= 1;
                let block = self.get_block(height).await?;
                blocks.push(block.clone());
            }
        }
        Ok(blocks)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct SubscanBlocksRes {
    code: u16,
    message: String,
    generated_at: u64,
    data: SubscanBlocks,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
struct SubscanBlocks {
    blocks: Vec<SubscanBlock>,
    count: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct SubscanBlock {
    block_num: u64,
    block_timestamp: u64,
    hash: String,
    event_count: u64,
    extrinsics_count: u64,
    finalized: bool,
    account_display: serde_json::Value,
}
impl SubscanBlock {
    pub fn to_block(&self) -> blockchain::Block {
        blockchain::Block {
            hash: self.hash.clone(),
            height: self.block_num,
            time: self.block_timestamp,
            txs: self.extrinsics_count,
        }
    }
}

#[cfg(test)]

mod tests {
    extern crate env_logger;
    use super::*;
    use crate::tests;
    use crate::utils;

    #[tokio::test]
    async fn subscan_parse_top_blocks() {
        tests::setup();
        let mut subscan = Subscan::test_new(
            "https://polkadot.api.subscan.io",
            Protocol::Polkadot,
            Network::Mainnet,
        );
        let blockchain = subscan.parse_top_blocks(5, None).await;
        assert!(blockchain.is_ok(), "Subscan should return a blockchain");
        let blockchain = blockchain.unwrap();
        let last_hash = blockchain.blocks.first().unwrap().hash.clone();
        assert!(
            blockchain.blocks.len() == 5 as usize,
            "Subscan should return 5 blocks but returned {}",
            blockchain.blocks.len()
        );
        utils::assert_blockchain(blockchain);
        // should return less blocks than requested (head is recent header)
        let blockchain = subscan.parse_top_blocks(40, Some(last_hash.clone())).await;
        if blockchain.is_ok() {
            let blockchain = blockchain.unwrap();
            assert!(
                blockchain.blocks.len() == 40,
                "Subscan should n block as expected, {} expected, {} returned",
                40,
                blockchain.blocks.len()
            );
        } else {
            match blockchain.err().unwrap().to_string().as_str() {
                "No new block" => {}
                _ => {
                    assert!(false, "Unexpected error");
                }
            }
        }
    }
}
