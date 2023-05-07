use async_trait::async_trait;

use serde::{Deserialize, Serialize};

use super::ProviderActions;
use crate::commons::blockchain;

use crate::conf::{self, Endpoint, EndpointActions, Network, Protocol};
use crate::requests::client::ReqwestClient;
use crate::requests::rpc::{
    JsonRpcParams, JsonRpcReq, JsonRpcReqBody, JsonRpcResponse, JSON_RPC_VER,
};
use crate::utils::deserialize_from_hex_to_u64;

#[derive(Serialize, Debug, Clone)]
pub struct TemplateNode {
    pub endpoint: conf::Endpoint,
}

impl TemplateNode {
    pub fn new(
        options: conf::EndpointOptions,
        protocol: Protocol,
        network: Network,
    ) -> TemplateNode {
        let endpoint = Endpoint {
            url: options.url.clone().unwrap(),
            reqwest: Some(ReqwestClient::new(options)),
            protocol,
            network,
            last_request: 0,
        };
        TemplateNode { endpoint }
    }
    #[cfg(test)]
    pub fn test_new(url: &str, proto: Protocol, net: crate::conf::Network) -> Self {
        TemplateNode {
            endpoint: conf::Endpoint::test_new(url, proto, net, None, None),
        }
    }
}
#[async_trait]
impl ProviderActions for TemplateNode {
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

        let previous_head = previous_head.unwrap_or("".to_string());

        blockchain.sort();
        let reqwest = self.endpoint.reqwest.as_mut().unwrap();
        reqwest.set_last_request();
        set_blockchain_height_endpoint(
            &self.endpoint.url,
            &self.endpoint.protocol,
            &self.endpoint.network,
            blockchain.height,
        );
        Ok(blockchain)
    }
}

impl TemplateNode {
    async fn some_call(&mut self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok("".to_string())
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
struct Derivethigs {
    header: TemplateBlockHeader,
    extrinsics: Vec<String>,
}

#[cfg(test)]

mod tests {
    extern crate env_logger;
    use super::*;
    use crate::tests;
    use crate::utils;
    use hex;

    #[tokio::test]
    async fn template_node_parse_top_blocks() {
        tests::setup();
        assert!(true);
    }
}
