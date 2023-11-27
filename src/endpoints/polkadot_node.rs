use async_trait::async_trait;

use serde::{Deserialize, Serialize};

use super::ProviderActions;
use crate::commons::blockchain;

use crate::conf::{self, Endpoint, Network, Protocol};
use crate::prom::registry::set_blockchain_height_endpoint;
use crate::requests::client::ReqwestClient;
use crate::requests::rpc::{
    JsonRpcParams, JsonRpcReq, JsonRpcReqBody, JsonRpcResponse, JSON_RPC_VER,
};
use crate::utils::deserialize_from_hex_to_u64;

#[derive(Serialize, Debug, Clone)]
pub struct PolkadotNode {
    pub endpoint: conf::Endpoint,
}

impl PolkadotNode {
    pub fn new(
        options: conf::EndpointOptions,
        protocol: Protocol,
        network: Network,
    ) -> PolkadotNode {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: ReqwestClient::new(options),
            protocol,
            network,
            last_request: 0,
        };
        PolkadotNode { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: crate::conf::Network) -> Self {
        PolkadotNode {
            endpoint: conf::Endpoint::test_new(url, proto, net, None, None),
        }
    }
}
#[async_trait]
impl ProviderActions for PolkadotNode {
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
        let head_hash = self.get_finalized_head().await?;

        let mut i = 0;
        let mut prev_hash = head_hash.clone();
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(None);
        while i < n_block {
            if previous_head == prev_hash {
                debug!(
                    "No new block (head: {} block with hash {}), skip task",
                    head_hash, prev_hash
                );
                return Err("No new block".into());
            }

            let block_res = self.get_blocks(vec![prev_hash.clone()]).await?;
            let block_res = block_res.get(0);
            let block = match block_res {
                Some(block) => &block.block,
                None => {
                    return Err("Get block return empty vec".into());
                }
            };
            let decode_timestamp = get_block_timestamp(block).unwrap_or(0);

            let b = blockchain::Block {
                hash: prev_hash.clone(),
                height: block.header.number,
                time: decode_timestamp,
                txs: block.extrinsics.len() as u64,
            };
            prev_hash = block.header.parent_hash.clone();
            i += 1;
            blockchain.add_block(b);
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

impl PolkadotNode {
    async fn get_finalized_head(
        &mut self,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let req = JsonRpcReq {
            jsonrpc: JSON_RPC_VER.to_string(),
            method: "chain_getFinalizedHead".to_string(),
            params: vec![],
            id: 1,
        };
        let req = JsonRpcReqBody::Single(req);
        let client = &mut self.endpoint.reqwest;
        let res: JsonRpcResponse<String> = client
            .rpc(&req, &self.endpoint.protocol, &self.endpoint.network)
            .await?;
        match res.result {
            Some(res) => Ok(res),
            None => Err("get_finalized_head return empty hash".into()),
        }
    }
    async fn get_blocks(
        &mut self,
        hashs: Vec<String>,
    ) -> Result<Vec<PolkadotBlockResponse>, Box<dyn std::error::Error + Send + Sync>> {
        if hashs.is_empty() {
            return Err("get_blocks: hashs is empty".into());
        };

        let mut batch = Vec::new();
        hashs.into_iter().for_each(|f| {
            let req = JsonRpcReq {
                jsonrpc: JSON_RPC_VER.to_string(),
                method: "chain_getBlock".to_string(),
                params: vec![JsonRpcParams::String(f)],
                id: 1,
            };
            batch.push(req);
        });
        let req = JsonRpcReqBody::Batch(batch);
        let client = &mut self.endpoint.reqwest;
        let res: Vec<JsonRpcResponse<PolkadotBlockResponse>> = client
            .rpc(&req, &self.endpoint.protocol, &self.endpoint.network)
            .await?;
        Ok(res.into_iter().filter_map(|f| f.result).collect())
    }
}

fn get_block_timestamp(block: &PolkadotBlock) -> Option<u64> {
    let timestamp_extrinsic = block.extrinsics[0].clone();
    let extrinsics = &timestamp_extrinsic[2..];
    let extrinsic_bytes = hex::decode(extrinsics).unwrap();
    let timestamp = decode_timestamp_extrinsic(&extrinsic_bytes)?;
    Some(timestamp)
}
// https://substrate.stackexchange.com/questions/2696/how-to-use-the-scale-decoder-to-parse-extrinsics
// TODO: use scale codec to decode extrinsic
// parity-scale-codec = {version= "3.4.0",  features = ["derive"] }
#[allow(clippy::needless_range_loop)]
fn decode_timestamp_extrinsic(extrinsic: &[u8]) -> Option<u64> {
    // Decode the compact u64 representing the timestamp
    let data = &extrinsic[4..];
    let length = data[0] & 0b11;
    if length != 0b11 {
        return None;
    }
    let mut value = 0u64;
    for i in 1..=6 {
        value += (data[i] as u64) << (8 * (i - 1));
    }
    Some(value)
}
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct PolkadotBlockHeader {
    parent_hash: String,
    #[serde(deserialize_with = "deserialize_from_hex_to_u64")]
    number: u64,
    state_root: String,
    extrinsics_root: String,
    digest: serde_json::Value,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
struct PolkadotBlock {
    header: PolkadotBlockHeader,
    extrinsics: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct PolkadotBlockResponse {
    block: PolkadotBlock,
    justifications: serde_json::Value,
}

#[cfg(test)]

mod tests {
    extern crate env_logger;
    use super::*;
    use crate::tests;
    use crate::utils;
    use hex;

    #[tokio::test]
    async fn polkadot_node_parse_top_blocks() {
        tests::setup();
        let mut endpoint = PolkadotNode::test_new(
            "https://rpc.polkadot.io",
            Protocol::Polkadot,
            String::from("mainnet"),
        );
        let res = endpoint.parse_top_blocks(10, None).await;
        assert!(res.is_ok());
        let blockchain = res.unwrap();
        utils::assert_blockchain(blockchain);
    }

    #[tokio::test]
    async fn polkadot_node_get_finalized_head() {
        tests::setup();
        let mut endpoint = PolkadotNode::test_new(
            "https://rpc.polkadot.io",
            Protocol::Polkadot,
            String::from("mainnet"),
        );
        let res = endpoint.get_finalized_head().await;
        assert!(res.is_ok());
        assert!(res.unwrap().starts_with("0x"));
    }
    #[tokio::test]
    async fn polkadot_node_get_block() {
        tests::setup();
        let mut endpoint = PolkadotNode::test_new(
            "https://rpc.polkadot.io",
            Protocol::Polkadot,
            String::from("mainnet"),
        );
        let res = endpoint
            .get_blocks(vec![
                "0xed74086309b9ac5e152188f5fbb6163a8f5fbb8b44f5b400a27e516386c478b6".to_string(),
            ])
            .await;
        assert!(res.is_ok());
        let res = res.unwrap();
        assert!(res.len() == 1);
        let block = res[0].clone();
        assert_eq!(
            block.block.header.parent_hash,
            "0x6a830b3dc9cf8b30100c6074a8c63b84679186646e972d79a7b2d5cf4e921baa"
        );
        let extrinsics = block.block.extrinsics[0].clone();
        let extrinsics = &extrinsics[2..];
        let extrinsic_bytes = hex::decode(extrinsics).unwrap();
        let r = decode_timestamp_extrinsic(&extrinsic_bytes[..]);
        assert!(r.is_some());
        let r = r.unwrap();
        assert!(r == 1682686764001);
    }
    #[tokio::test]
    async fn polkadot_node_decode_timestamp_extrinsics() {
        tests::setup();
        let mut endpoint = PolkadotNode::test_new(
            "https://rpc.polkadot.io",
            Protocol::Polkadot,
            String::from("mainnet"),
        );
        let block_hashs = vec![
            "0xf752e2ec759fc921b7fb63f5c4c3798ed608059dc239dfb2f2d41bb50190dd35".to_string(),
            "0xe8018641a76511fabe2625768315191a4708ec535ee0c58a13ab8ded8b6e9dd9".to_string(),
            "0x4ed9d382d06717b818ae8a9d24f82842a56efac1d39ae9ab09b1a9fb29fbc229".to_string(),
            "0xf250c96cfe3a8281d9520a904c6ea77dbee2f4f71cd476116cc515c980e90fb3".to_string(),
            "0x7363a5134e66b051af633c8d5ce81e61768379df26bca9f90e26674f4efd3a5d".to_string(),
            "0x8cc9b0c66d1d742f8eb978c1ce51cff96004f291c0d311cc16ed4c48860e05d7".to_string(),
            "0x721c3890a18ed9d5595aa599e3241a269d2da29d53ac004b172ca262768cb8a6".to_string(),
        ];
        let assert_timestamps: Vec<u64> = vec![
            1682756880000,
            1682756874000,
            1682756856000,
            1682756850001,
            1682756844000,
            1676704524000,
            1664682306006,
        ];
        let res = endpoint.get_blocks(block_hashs).await;
        assert!(res.is_ok());
        for (i, r) in res.unwrap().iter().enumerate() {
            let extrinsics = r.block.extrinsics[0].clone();
            let extrinsics = &extrinsics[2..];
            let extrinsic_bytes = hex::decode(extrinsics).unwrap();
            let r = decode_timestamp_extrinsic(&extrinsic_bytes[..]);
            assert!(r.is_some());
            let r = r.unwrap();
            assert!(r == assert_timestamps[i]);
        }
    }
}
