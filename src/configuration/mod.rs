use std::{collections::HashMap, str::FromStr};

use config::{self, ConfigError, File};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::Value;

use crate::{endpoints::{bitcoin_node::BitcoinNode, blockstream::{Blockstream}}, requests::client::ReqwestClient};

pub static CONFIGURATION: OnceCell<Configuration> = OnceCell::new();
pub static CONFIGURATION_GLOB_ENDPOINT_OPTION: OnceCell<EndpointOptions> = OnceCell::new();
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Database {
    pub keep_history: u32,
}

#[derive(Serialize, Debug, Clone)]
pub struct Configuration {
    pub global: Global,
    pub database : Database,
    #[serde(deserialize_with = "deserialize_protocols")]
    pub protocols: HashMap<ProtocolName,HashMap<NetworkName,ProtocolsOpts>>,
}
// Deserialize configuration should be used to be sure global configuration will be deserialized first
impl <'de>Deserialize<'de> for Configuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        debug!("deserialize_configuration");
        let v: Value = Deserialize::deserialize(deserializer)?;

        let global = Global::deserialize(v.as_object().unwrap().get("global").unwrap()).unwrap();
        let database = Database::deserialize(v.as_object().unwrap().get("database").unwrap()).unwrap();
        let protocols = deserialize_protocols(v.as_object().unwrap().get("protocols").unwrap()).unwrap();
        Ok(Configuration {
            global,
            database,
            protocols,
        })
    }
}

fn deserialize_protocols<'de, D>(deserializer: D) -> Result<HashMap<ProtocolName, HashMap<NetworkName,ProtocolsOpts>>, D::Error>
where
    D: Deserializer<'de>,
{
    debug!("deserialize_protocols");
    let v: Value = Deserialize::deserialize(deserializer)?;
    let mut map = HashMap::new();
    for (proto_k, v) in v.as_object().unwrap() {
        let proto_k = ProtocolName::from_str(proto_k).unwrap();
        for (k, v) in v.as_object().unwrap() {
            let k = NetworkName::from_str(k).unwrap();          
            match proto_k {
                ProtocolName::Bitcoin => {
                    debug!("deserialize bitcoin {:?}", v);
                    let mut bitcoin_opts = BitcoinOpts::default();
                    for (endpoint_param_name, endpoint_obj) in v.as_object().unwrap() {
                        match endpoint_param_name.as_str() {
                            "network_options"=>{
                                let network_opts = NetworkOptions::deserialize(endpoint_obj).unwrap();
                                bitcoin_opts.network_options = Some(network_opts);
                            },
                            "rpc" =>{
                                let rpc_endpts : Vec<Endpoint> =serde_json::from_str(&endpoint_obj.to_string()).unwrap();
                                let bitcoin_nodes = rpc_endpts.iter().map(| e| {
                                    let mut e = e.clone();
                                    e.network = k.clone();
                                    BitcoinNode::new(e.clone())
                                }).collect();
                                bitcoin_opts.rpc = Some(bitcoin_nodes);
                            }
                            _ => {
                                let mut endpoint = Endpoint::deserialize(endpoint_obj).unwrap();
                                endpoint.network = k.clone();
                                let endpoint_param_name = BitcoinAvailableEndpoints::from_str(endpoint_param_name);
                                match endpoint_param_name {
                                    Ok(BitcoinAvailableEndpoints::Blockstream) => {
                                        let blockstream = Blockstream::new(endpoint);
                                        bitcoin_opts.blockstream = Some(blockstream);
                                    }
                                    _ => {
                                        error!("endpoint_param_name: {:?}", endpoint_param_name);

                                    }
                                }                                
                            }
                        }
                    }

                    map.entry(proto_k.clone()).or_insert(HashMap::new()).insert(k, ProtocolsOpts::Bitcoin(bitcoin_opts));
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
    #[serde(deserialize_with = "deserialize_global_endpoint_options")]
    pub endpoints: EndpointOptions,
    pub metrics: Metrics,
    pub server: Server,
    pub networks_options: NetworkOptions,
}
// Little trick to init global endpoint options, this Global will be used in the rest of deserialization process to init request client
fn deserialize_global_endpoint_options<'de, D>(deserializer: D) -> Result<EndpointOptions, D::Error>
where
    D: Deserializer<'de>,
{
    let endpoint_opt = EndpointOptions::deserialize(deserializer).unwrap();
    debug!("deserialize_global_endpoint_options: {:?}", endpoint_opt);
    CONFIGURATION_GLOB_ENDPOINT_OPTION.set(endpoint_opt.clone()).unwrap();
    debug!("set CONFIGURATION_GLOB_ENDPOINT_OPTION: {:?}", CONFIGURATION_GLOB_ENDPOINT_OPTION.get().unwrap());
    Ok(endpoint_opt)
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
    Ghostnet,
    #[serde(rename = "")]
    InitState
}
impl NetworkName {
    pub fn to_string(&self) -> String {
        match self {
            NetworkName::Mainnet => "mainnet".to_string(),
            NetworkName::Testnet => "testnet".to_string(),
            NetworkName::Goerli => "goerli".to_string(),
            NetworkName::Sepolia => "sepolia".to_string(),
            NetworkName::Ghostnet => "ghostnet".to_string(),
            NetworkName::InitState => "".to_string()
        }
    }
}
impl FromStr for NetworkName {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(NetworkName::Mainnet),
            "testnet" => Ok(NetworkName::Testnet),
            "goerli" => Ok(NetworkName::Goerli),
            "sepolia" => Ok(NetworkName::Sepolia),
            "ghostnet" => Ok(NetworkName::Ghostnet),
            ""=> Ok(NetworkName::InitState),
            _ => Err(format!("{} is not a valid network name", s)),
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
impl FromStr for ProtocolName {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "bitcoin" => Ok(ProtocolName::Bitcoin),
            "ethereum" => Ok(ProtocolName::Ethereum),
            "tezos" => Ok(ProtocolName::Tezos),
            _ => Err(format!("{} is not a valid protocol name", s)),
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

#[derive(Deserialize, Serialize, Debug,Clone,Default)]
pub struct BitcoinOpts { 
    pub network_options: Option<NetworkOptions>,
    pub rpc: Option<Vec<BitcoinNode>>,
    pub blockstream: Option<Blockstream>,
    pub blockcypher: Option<Endpoint>,
}
#[derive(Deserialize, Serialize, Debug,Clone)]
pub enum BitcoinAvailableEndpoints {
    Blockstream,
    Blockcypher,
}
impl BitcoinAvailableEndpoints {
    pub fn to_string(&self) -> String {
        match self {
            BitcoinAvailableEndpoints::Blockstream => "blockstream".to_string(),
            BitcoinAvailableEndpoints::Blockcypher => "blockcypher".to_string(),
        }
    }
}
impl std::fmt::Display for BitcoinAvailableEndpoints {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BitcoinAvailableEndpoints::Blockstream => write!(f, "blockstream"),
            BitcoinAvailableEndpoints::Blockcypher => write!(f, "blockcypher"),
        }
    }
}
impl FromStr for BitcoinAvailableEndpoints {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "blockstream" => Ok(BitcoinAvailableEndpoints::Blockstream),
            "blockcypher" => Ok(BitcoinAvailableEndpoints::Blockcypher),
            _ => Err(format!("Unknown bitcoin endpoint: {}", s)),
        }
    }
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
#[derive( Serialize, Debug, Clone)]
pub struct Endpoint {
    pub url: String,
    // pub options: Option<EndpointOptions>,
    #[serde(skip)]
    pub reqwest: Option<ReqwestClient>,
    #[serde(skip)]
    pub network: NetworkName,
    #[serde(skip)]
    pub last_request: u64,
}
// Endpoint deserialization will substitute options field, to init reqwest client
impl<'de>Deserialize<'de> for Endpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        error!("deserialize endpoint");
        let v: Value = Deserialize::deserialize(deserializer)?;
        let url = v.get("url").ok_or_else(|| serde::de::Error::custom("Missing url"))?.as_str().ok_or_else(|| serde::de::Error::custom("Invalid url"))?.to_string();
        let options = v.get("options");
        let options = match options {
            Some(options) => {
                let mut options = serde_json::from_value::<EndpointOptions>(options.clone()).map_err(|e| serde::de::Error::custom(format!("Invalid options: {}", e)))?;
                options.init(Some(&url));
                options
            },
            None => {
                EndpointOptions::default()
            }
        };

        Ok(Endpoint {
            url,
            // options:Some(options.clone()),
            reqwest: Some(ReqwestClient::new(options)),
            network: NetworkName::InitState,
            last_request: 0,
        })

        
    }
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
        let global_endpoint_conf = CONFIGURATION_GLOB_ENDPOINT_OPTION.get().unwrap().clone();
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
        debug!("get CONFIGURATION_GLOB_ENDPOINT_OPTION endpoint options ");
        let global_endpoint_conf = CONFIGURATION_GLOB_ENDPOINT_OPTION.get().unwrap().clone();
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
