use async_trait::async_trait;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use super::ProviderActions;
use crate::commons::blockchain::{self, Block};

use crate::conf::{self, Endpoint, EndpointActions, Network, Protocol};
use crate::requests::client::ReqwestClient;

#[derive(Serialize, Debug, Clone)]
pub struct TzStats {
    pub endpoint: conf::Endpoint,
}

impl TzStats {
    pub fn new(options: conf::EndpointOptions, protocol: Protocol, network: Network) -> TzStats {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: Some(ReqwestClient::new(options)),
            protocol,
            network,
            last_request: 0,
        };
        TzStats { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: Network) -> Self {
        TzStats {
            endpoint: conf::Endpoint::test_new(url, proto, net),
        }
    }
    async fn get_block(
        &mut self,
        height: Option<String>,
    ) -> Result<TzStatsBlock, Box<dyn std::error::Error + Send + Sync>> {
        let q = height.unwrap_or("head".to_string());
        let url = format!("{}/explorer/block/{}", self.endpoint.url, q);
        let client = self.endpoint.reqwest.as_mut().unwrap();
        let head: TzStatsBlock = client
            .get(
                &url,
                &Protocol::Tezos.to_string(),
                &self.endpoint.network.to_string(),
            )
            .await?;
        Ok(head)
    }
}
#[async_trait]
impl ProviderActions for TzStats {
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
        if !self.endpoint.available() {
            return Err("Error: Endpoint not available".into());
        }
        let head = self.get_block(None).await?;
        let previous_head: String = previous_head.unwrap_or("".to_string());
        if previous_head == head.hash {
            debug!(
                "No new block (head: {} block with hash {}), skip task",
                head.height, head.hash
            );
            self.endpoint.set_last_request();
            return Err("No new block".into());
        }
        let head_block = Block {
            hash: head.hash,
            height: head.height,
            time: DateTime::parse_from_rfc3339(&head.time)?.timestamp_millis() as u64,
            txs: head.n_tx,
        };
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(None);
        blockchain.add_block(head_block);
        let mut i = 1;
        let mut seach_height = head.predecessor.unwrap();
        while i < n_block {
            let r = self.get_block(Some(seach_height.clone())).await?;
            let block = Block {
                hash: r.hash,
                height: r.height,
                time: DateTime::parse_from_rfc3339(&r.time)?.timestamp_millis() as u64,
                txs: r.n_tx,
            };
            if block.hash == previous_head {
                debug!("Previous head found, stop parsing blocks");
                break;
            }
            seach_height = r.predecessor.unwrap();
            blockchain.add_block(block);
            i += 1;
        }
        blockchain.sort();
        Ok(blockchain)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct TzStatsBlock {
    hash: String,
    predecessor: Option<String>,
    successor: Option<String>,
    protocol: String,
    baker: String,
    proposer: String,
    baker_consensus_key: String,
    proposer_consensus_key: String,
    height: u64,
    cycle: u32,
    is_cycle_snapshot: bool,
    time: String,
    solvetime: u32,
    version: u32,
    round: u32,
    nonce: String,
    voting_period_kind: String,
    n_endorsed_slots: u32,
    n_ops_applied: u32,
    n_ops_failed: u32,
    n_events: u32,
    n_calls: u32,
    n_rollup_calls: u32,
    n_tx: u64,
    volume: f64,
    fee: f64,
    reward: f64,
    deposit: f64,
    activated_supply: f64,
    minted_supply: f64,
    burned_supply: f64,
    n_accounts: u32,
    n_new_accounts: u32,
    n_new_contracts: u32,
    n_cleared_accounts: u32,
    n_funded_accounts: u32,
    gas_limit: u32,
    gas_used: u32,
    storage_paid: u32,
    pct_account_reuse: f64,
    lb_esc_vote: String,
    lb_esc_ema: u32,
    // Should be specified in request
    metadata: Option<serde_json::Value>,
    rights: Option<Vec<serde_json::Value>>,
}
#[cfg(test)]
mod tests {

    extern crate env_logger;
    use super::*;
    use crate::tests;
    #[tokio::test]
    async fn tzstats_parse_top_blocks() {
        tests::setup();
        let url = "https://api.ghost.tzstats.com";
        let mut tzstats = TzStats::test_new(url, Protocol::Tezos, Network::Ghostnet);
        let blockchain = tzstats
            .parse_top_blocks(5, None)
            .await
            .expect("parse_top_blocks");
        assert_eq!(blockchain.blocks.len(), 5);

        let height_start = blockchain.blocks[0].height;
        for i in 1..blockchain.blocks.len() {
            assert_eq!(blockchain.blocks[i].height, height_start - i as u64);
        }
    }
}
