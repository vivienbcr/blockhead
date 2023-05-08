use async_trait::async_trait;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use super::ProviderActions;
use crate::commons::blockchain;

use crate::conf::{self, Endpoint, Network, Protocol};
use crate::prom::registry::set_blockchain_height_endpoint;
use crate::requests::client::ReqwestClient;

#[derive(Serialize, Debug, Clone)]
pub struct Tzkt {
    pub endpoint: conf::Endpoint,
}

impl Tzkt {
    pub fn new(options: conf::EndpointOptions, protocol: Protocol, network: Network) -> Tzkt {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
            protocol,
            network,
            last_request: 0,
        };
        Tzkt { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: crate::conf::Network) -> Self {
        Tzkt {
            endpoint: conf::Endpoint::test_new(url, proto, net, None, None),
        }
    }
}
#[async_trait]
impl ProviderActions for Tzkt {
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

        let head = self.get_head().await?;
        if previous_head == head.hash {
            debug!(
                "No new block (head: {} block with hash {}), skip task",
                head.level, head.hash
            );
            return Err("No new block".into());
        }
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(None);
        let mut i = 0;
        let head_level = head.level;
        while i < n_block {
            let block = self.get_block_full(head_level - i).await?;
            if block.hash == previous_head {
                debug!("Previous head found, stop parsing blocks");
                break;
            }
            let datetime = DateTime::parse_from_rfc3339(&block.timestamp).unwrap();
            let timestamp = datetime.timestamp();
            let b = blockchain::Block {
                height: block.level,
                hash: block.hash,
                time: timestamp as u64,
                txs: block.transactions.len() as u64,
            };

            blockchain.add_block(b);
            i += 1;
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

impl Tzkt {
    async fn get_head(&mut self) -> Result<TzktHead, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/v1/blocks?sort.desc=level&select=level,hash&limit=1",
            self.endpoint.url,
        );
        let client = &mut self.endpoint.reqwest;
        let res: Vec<TzktHead> = client
            .run_request(
                reqwest::Method::GET,
                None,
                &url,
                &self.endpoint.protocol,
                &self.endpoint.network,
            )
            .await?;
        if res.len() == 0 {
            return Err("Error: No head found".into());
        }
        Ok(res[0].clone())
    }
    // get_block_full return block object will all operations
    async fn get_block_full(
        &mut self,
        block_level: u32,
    ) -> Result<TzktBlockFull, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/v1/blocks/{}?operations=true",
            self.endpoint.url, block_level
        );
        let client = &mut self.endpoint.reqwest;
        let res: TzktBlockFull = client
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

#[derive(Deserialize, Serialize, Debug, Clone)]
struct TzktHead {
    level: u32,
    hash: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct TzktBlockFull {
    pub cycle: u32,
    pub level: u64,
    pub hash: String,
    pub timestamp: String,
    pub proto: u32,
    pub payload_round: u32,
    pub block_round: u32,
    pub validations: u32,
    pub deposit: u32,
    pub reward: u32,
    pub bonus: u32,
    pub fees: u32,
    pub nonce_revealed: bool,
    pub proposer: serde_json::Value,
    pub producer: serde_json::Value,
    pub software: serde_json::Value,
    pub lb_toggle_ema: u32,
    pub endorsements: Vec<serde_json::Value>,
    pub preendorsements: Vec<serde_json::Value>,
    pub proposals: Vec<serde_json::Value>,
    pub ballots: Vec<serde_json::Value>,
    pub activations: Vec<serde_json::Value>,
    pub double_baking: Vec<serde_json::Value>,
    pub double_endorsing: Vec<serde_json::Value>,
    pub double_preendorsing: Vec<serde_json::Value>,
    pub nonce_revelations: Vec<serde_json::Value>,
    pub vdf_revelations: Vec<serde_json::Value>,
    pub delegations: Vec<serde_json::Value>,
    pub originations: Vec<serde_json::Value>,
    pub transactions: Vec<serde_json::Value>,
    pub reveals: Vec<serde_json::Value>,
    pub register_constants: Vec<serde_json::Value>,
    pub set_deposits_limits: Vec<serde_json::Value>,
    pub transfer_ticket_ops: Vec<serde_json::Value>,
    pub tx_rollup_commit_ops: Vec<serde_json::Value>,
    pub tx_rollup_dispatch_tickets_ops: Vec<serde_json::Value>,
    pub tx_rollup_finalize_commitment_ops: Vec<serde_json::Value>,
    pub tx_rollup_origination_ops: Vec<serde_json::Value>,
    pub tx_rollup_rejection_ops: Vec<serde_json::Value>,
    pub tx_rollup_remove_commitment_ops: Vec<serde_json::Value>,
    pub tx_rollup_return_bond_ops: Vec<serde_json::Value>,
    pub tx_rollup_submit_batch_ops: Vec<serde_json::Value>,
    pub increase_paid_storage_ops: Vec<serde_json::Value>,
    pub update_consensus_key_ops: Vec<serde_json::Value>,
    pub drain_delegate_ops: Vec<serde_json::Value>,
    pub sr_add_messages_ops: Vec<serde_json::Value>,
    pub sr_cement_ops: Vec<serde_json::Value>,
    pub sr_execute_ops: Vec<serde_json::Value>,
    pub sr_originate_ops: Vec<serde_json::Value>,
    pub sr_publish_ops: Vec<serde_json::Value>,
    pub sr_recover_bond_ops: Vec<serde_json::Value>,
    pub sr_refute_ops: Vec<serde_json::Value>,
    pub migrations: Vec<serde_json::Value>,
    pub revelation_penalties: Vec<serde_json::Value>,
    pub endorsing_rewards: Vec<serde_json::Value>,
    pub priority: u32,
    pub baker: serde_json::Value,
    pub lb_escape_vote: bool,
    pub lb_escape_ema: u32,
}

#[cfg(test)]

mod tests {
    extern crate env_logger;
    use super::*;
    use crate::tests;
    #[tokio::test]
    async fn tzkt_get_block_full() {
        tests::setup();
        let url = "https://api.ghostnet.tzkt.io";
        let mut tzkt = Tzkt::test_new(url, Protocol::Tezos, Network::Ghostnet);
        let r = tzkt.get_block_full(123456).await.unwrap();
        assert_eq!(r.level, 123456);
    }
    #[tokio::test]
    async fn tzkt_get_head() {
        tests::setup();
        let url = "https://api.ghostnet.tzkt.io";
        let mut tzkt = Tzkt::test_new(url, Protocol::Tezos, Network::Ghostnet);
        let r = tzkt.get_head().await.unwrap();
        assert!(r.level > 123456);
    }
    #[tokio::test]
    async fn tzkt_parse_top() {
        tests::setup();
        let url = "https://api.ghostnet.tzkt.io";
        let mut tzkt = Tzkt::test_new(url, Protocol::Tezos, Network::Ghostnet);
        let r = tzkt.parse_top_blocks(5, None).await.unwrap();
        assert_eq!(r.blocks.len(), 5);
    }
}
