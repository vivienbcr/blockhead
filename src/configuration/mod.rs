use std::{collections::HashMap, error::Error};

use config::{self, ConfigError, File};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::Value;

use crate::{endpoints::{bitcoin_node::BitcoinNode, blockstream::Blockstream}};

pub static CONFIGURATION: OnceCell<Configuration> = OnceCell::new();

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Configuration {
    pub global: Global,
    #[serde(deserialize_with = "deserialize_protocols")]
    pub protocols: HashMap<ProtocolName,HashMap<NetworkName,ProtocolsOpts>>,
}
fn deserialize_protocols<'de, D>(deserializer: D) -> Result<HashMap<ProtocolName, HashMap<NetworkName,ProtocolsOpts>>, D::Error>
where
    D: Deserializer<'de>,
{
    debug!("deserialize_protocols");
    let v: Value = Deserialize::deserialize(deserializer)?;
    let mut map = HashMap::new();
    for (proto_k, v) in v.as_object().unwrap() {
        let proto_k = match proto_k.as_str() {
            "bitcoin" => ProtocolName::Bitcoin,
            "ethereum" => ProtocolName::Ethereum,
            "tezos" => ProtocolName::Tezos,
            _ => return Err(serde::de::Error::custom(format!("Unknown protocol: {}", proto_k))), 
        };
        for (k, v) in v.as_object().unwrap() {
            let k = match k.as_str() {
                "mainnet" => NetworkName::Mainnet,
                "testnet" => NetworkName::Testnet,
                "goerli" => NetworkName::Goerli,
                "sepolia" => NetworkName::Sepolia,
                "ghostnet" => NetworkName::Ghostnet,
                _ => return Err(serde::de::Error::custom(format!("Unknown or unsupported network: {} for protocol {}", k, proto_k))),
            };
            // find option field in v
            // if option field is present, init request client
            // println!("v: {:?}", v);
            // let enpoints_list : Vec<_> = v.as_object().unwrap().keys().collect();
            // let mut rpc_endpoints: Vec<EndpointOptions> = Vec::new();
            // for e in enpoints_list {
            //     match e.as_str() {
            //         "rpc" => {
            //             println!("rpc");
            //             let rpc_conf = v.as_object().unwrap().get("rpc").unwrap();
            //             rpc_endpoints  = serde_json::from_value(rpc_conf.clone()).unwrap();
            //             println!("rpc_conf: {:?}", rpc_conf);
            //         }
            //         _ =>{
            //             println!("e: {:?}", e);
            //         }
            //     }
            // }
          
            // it would be usefull to init request client here
            match proto_k {
                ProtocolName::Bitcoin => {
                    let endpoints = BitcoinOpts::deserialize(v).unwrap();
                    println!("endpoints setp 0: {:?}", endpoints);
                    // if endpoints.rpc.is_some() {
                    //     // set network value in endpoints.rpc
                    //     endpoints.rpc.unwrap().network = k.clone();

                    // }
                    // println!("endpoints setp 1: {:?}", endpoints);
                    // if endpoints.rpc.is_some() {

                    map.entry(proto_k.clone()).or_insert(HashMap::new()).insert(k, ProtocolsOpts::Bitcoin(endpoints));
                },
                ProtocolName::Ethereum => {
                    let endpoints = EthereumEndpoints::deserialize(v).unwrap();
                    map.entry(proto_k.clone()).or_insert(HashMap::new()).insert(k, ProtocolsOpts::Ethereum(endpoints));
                },
                ProtocolName::Tezos => {
                    let endpoints = TezosEndpoints::deserialize(v).unwrap();
                    map.entry(proto_k.clone()).or_insert(HashMap::new()).insert(k, ProtocolsOpts::Tezos(endpoints));
                },
            }
        }
    }
    Ok(map)
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Global {
    pub endpoints: EndpointOptions,
    pub metrics: Metrics,
    pub server: Server,
    pub networks_options: NetworkOptions,
    // pub head_length: u32,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Metrics {
    pub port: u16,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Server {
    pub port: u32,
}
#[derive(Deserialize, Serialize, Debug, Clone,Eq, Hash, PartialEq)]
pub enum NetworkName {
    #[serde(rename = "mainnet")]
    Mainnet,
    #[serde(rename = "testnet")]
    Testnet,
    #[serde(rename = "goerli")]
    Goerli,
    #[serde(rename = "sepolia")]
    Sepolia,
    #[serde(rename = "ghostnet")]
    Ghostnet
}
impl NetworkName {
    pub fn to_string(&self) -> String {
        match self {
            NetworkName::Mainnet => "mainnet".to_string(),
            NetworkName::Testnet => "testnet".to_string(),
            NetworkName::Goerli => "goerli".to_string(),
            NetworkName::Sepolia => "sepolia".to_string(),
            NetworkName::Ghostnet => "ghostnet".to_string(),
        }
    }
}


#[derive(Serialize, Debug, Clone,Eq, Hash, PartialEq)]
pub enum ProtocolName {
    #[serde(rename = "bitcoin")]
    Bitcoin,
    #[serde(rename = "ethereum")]
    Ethereum,
    #[serde(rename = "tezos")]
    Tezos,
}

impl std::fmt::Display for ProtocolName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ProtocolName::Bitcoin => write!(f, "bitcoin"),
            ProtocolName::Ethereum => write!(f, "ethereum"),
            ProtocolName::Tezos => write!(f, "tezos"),
        }
    }
}

impl<'de>Deserialize<'de> for ProtocolName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        debug!("s: {:?}", s);
        match s.as_str() {
            "bitcoin" => Ok(ProtocolName::Bitcoin),
            "ethereum" => Ok(ProtocolName::Ethereum),
            "tezos" => Ok(ProtocolName::Tezos),
            _ => Err(serde::de::Error::custom(format!("Unknown protocol: {}", s))),
        }
    }
}

#[derive( Serialize,Deserialize, Debug, Clone)]
pub enum ProtocolsOpts {
    Bitcoin(BitcoinOpts),
    Ethereum(EthereumEndpoints),
    Tezos(TezosEndpoints),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NetworkOptions {
    pub head_length: Option<u32>,
}
impl NetworkOptions {
    pub fn init(&mut self){
        let global_config = CONFIGURATION.get().unwrap();
        let global_networks_options = global_config.global.networks_options.clone();
        if self.head_length.is_none() {
            self.head_length = global_networks_options.head_length;
        }
    }
    pub fn default() -> Self {
        NetworkOptions {
            head_length: Some(DEFAULT_HEAD_LENGTH),
        }
    }
}

#[derive(Deserialize, Serialize, Debug,Clone)]
pub struct BitcoinOpts { 
    pub network_options: Option<NetworkOptions>,
    pub rpc: Option<Vec<BitcoinNode>>,
    pub blockstream: Option<Blockstream>,
    pub blockcypher: Option<Endpoint>,
}


#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EthereumEndpoints {
    pub rpc: Option<Vec<Endpoint>>,
    pub infura: Option<Endpoint>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TezosEndpoints {
    pub rpc: Option<Vec<Endpoint>>,
    pub tzstats: Option<Endpoint>,
    pub tzkt: Option<Endpoint>,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Endpoint {
    pub url: String,
    pub options: Option<EndpointOptions>,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EndpointOptions {
    pub url: Option<String>,
    pub retry: Option<u32>,
    pub delay: Option<u32>,
    pub rate: Option<u32>,
}
impl EndpointOptions {
    pub fn init(&mut self, url : Option<&str>){
        if url.is_some() {
            self.url = Some(url.unwrap().to_string());
        }
        let global_endpoint_conf = CONFIGURATION.get().unwrap().global.endpoints.clone();
        if self.retry.is_none() {
            self.retry = global_endpoint_conf.retry;
        }
        if self.delay.is_none() {
            self.delay = global_endpoint_conf.delay;
        }
        if self.rate.is_none() {
            self.rate = global_endpoint_conf.rate;
        }
    }
}
impl Default for EndpointOptions {
    fn default() -> Self {
        let global_endpoint_conf = CONFIGURATION.get().unwrap().global.endpoints.clone();
        EndpointOptions {
            url: None,
            retry: global_endpoint_conf.retry,
            delay: global_endpoint_conf.delay, 
            rate:  global_endpoint_conf.rate,
        } 
    }
}

pub const DEFAULT_SERVER_PORT: u32 = 8080;
pub const DEFAULT_METRICS_PORT: u16 = 8081;
pub const DEFAULT_HEAD_LENGTH: u32 = 5;
pub const DEFAULT_ENDPOINT_RETRY: u32 = 3;
pub const DEFAULT_ENDPOINT_DELAY: u32 = 1;
pub const DEFAULT_ENDPOINT_REQUEST_RATE: u32 = 5;

impl Configuration {
    pub fn new() -> Result<Self, ConfigError> {
        // TODO: config file should be overridable by env variables
        // TODO: config file should be overridable by cli args
        let builder = config::Config::builder()
        .set_default("global.server.port", DEFAULT_SERVER_PORT)?
        .set_default("global.metrics.port", DEFAULT_METRICS_PORT)?
        .set_default("global.networks_options.head_length", DEFAULT_HEAD_LENGTH)?
        .set_default("global.endpoints.retry", DEFAULT_ENDPOINT_RETRY)?
        .set_default("global.endpoints.delay", DEFAULT_ENDPOINT_DELAY)?
        .set_default("global.endpoints.rate", DEFAULT_ENDPOINT_REQUEST_RATE)?
            .add_source(File::with_name("config.yaml"))
            .build()?;
        let r: Result<Configuration, ConfigError> = builder.try_deserialize();
        match r {
            Ok(config) => {
                CONFIGURATION.set(config.clone()).unwrap();
                Ok(config)
            }
            Err(e) => Err(e),
        }
    }
    pub fn get_global_endpoint_config(&self) -> &EndpointOptions {
        &self.global.endpoints
    }
}
