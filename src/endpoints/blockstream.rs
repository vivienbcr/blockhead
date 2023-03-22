use std::time::{UNIX_EPOCH, SystemTime};

use async_trait::async_trait;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{configuration::{EndpointOptions, self}, requests::client::ReqwestClient, commons::blockchain};

use super::Endpoint;

#[derive(Deserialize, Serialize,Debug,Clone)]
pub struct Blockstream {
    pub url: String,
    pub options: Option<EndpointOptions>,
    #[serde(skip)]
    pub reqwest: Option<ReqwestClient>,
    #[serde(skip)]
    pub network: String,
    #[serde(skip)]
    pub last_request: u64,
}
#[async_trait]
impl Endpoint for Blockstream {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
        // Init reqwest client
        // Endpoint scope options override global options
        let default_endpoint_opts = configuration::CONFIGURATION.get().unwrap().get_global_endpoint_config();
        match &self.options {
            Some(opts) => {
                let endpoint_opt = Some(EndpointOptions {
                    url: Some(self.url.clone()),
                    retry: opts.retry.or(default_endpoint_opts.retry),
                    rate: opts.rate.or(default_endpoint_opts.rate),
                    delay : opts.delay.or(default_endpoint_opts.delay)
                });
                self.reqwest = Some(ReqwestClient::new(endpoint_opt.clone().unwrap()));
            }
            None => {
                let endpoint_opt = Some(EndpointOptions {
                    url: Some(self.url.clone()),
                    retry: default_endpoint_opts.retry,
                    rate: default_endpoint_opts.rate,
                    delay : default_endpoint_opts.delay
                });
                self.reqwest = Some(ReqwestClient::new(endpoint_opt.clone().unwrap()));
            }
        }
        debug!("Initialized Blockstream endpoint: {:?}", self);
        Ok(())
    }
    async fn parse_top_blocks(
        &mut self,
        network: &str,
         nb_blocks: u32
        ) -> Result<blockchain::Blockchain, Box<dyn std::error::Error + Send + Sync>> {
        let mut blockchain: blockchain::Blockchain = blockchain::Blockchain::new(
                &configuration::ProtocolName::Bitcoin.to_string(),
                network,
            );
        let mut height = self.get_chain_height().await?;
        let mut blocks = self.get_blocks_from_height(height).await?;
        while blocks.len() > 0 && blockchain.blocks.len() < nb_blocks as usize {
            for block in blocks {
                blockchain.blocks.push(blockchain::Block { hash: block.id, height: block.height, time: block.mediantime, txs: block.tx_count });
            }
            height = height - 10;
            blocks = self.get_blocks_from_height(height).await?;
        }
        self.set_last_request();
        blockchain.sort();
        Ok(blockchain)

    }
    fn available(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        let diff = now - self.last_request;
        if diff < self.reqwest.clone().unwrap().config.rate.unwrap() as u64 {
            debug!("Rate limit reached for {} ({}s)", self.url, diff);
            return false;
        }
        true
    }
}


impl Blockstream {
    pub fn new(
        endpoint_options: EndpointOptions,
        network: String,
    )-> Blockstream{
        Blockstream {
            url: endpoint_options.clone().url.unwrap(),
            options: Some(endpoint_options.clone()),
            reqwest: Some(ReqwestClient::new(endpoint_options.clone())),
            network,
            last_request: 0,
        }
    }
    fn set_last_request(&mut self){
        trace!("Set last request for {} to {}", self.url, self.last_request);
        self.last_request = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
    }
    // get_block return last 10 blocks
    async fn get_blocks_from_height(&self, height: u32) -> Result<Vec<Block>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/blocks/{}", self.url, height);
        self.run(&url).await
    }

    async fn get_chain_height(&self) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/blocks/tip/height", self.url);
        self.run(&url).await
    }

    async fn run<T: DeserializeOwned>(
        &self,
        url: &str
    )-> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let reqwest = self.reqwest.clone().unwrap();
        let res = reqwest.get(url,
            &configuration::ProtocolName::Bitcoin.to_string(),
        &self.network).await;
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                debug!("Error Blockstream: {}", e);
                return Err("Error: reqwest".into());
            }
        };
        let res = serde_json::from_str::<T>(&res);
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                debug!("Error Blockstream: deserialize json response {}", e);
                return Err("Error: serde_json".into());
            }
        };
        return Ok(res)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Block {
    pub id : String,
    pub height : u64,
    pub version : u64,
    pub timestamp : u64,
    pub tx_count : u64,
    pub size : u64,
    pub weight : u64,
    pub merkle_root : String,
    pub previousblockhash : String,
    pub mediantime : u64,
    pub nonce : u64,
    pub bits : u64,
    pub difficulty : u64,
}