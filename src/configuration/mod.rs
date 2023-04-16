use std::{
    collections::HashMap,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use config::{self, ConfigError, File};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::{
    conf2::EndpointOptions,
    endpoints::{
        bitcoin_node::BitcoinNode, blockcypher::Blockcypher, blockstream::Blockstream,
        ethereum_node::EthereumNode, ProviderActions,
    },
    requests::client::ReqwestClient,
};

pub static CONFIGURATION: OnceCell<Configuration> = OnceCell::new();
pub static CONFIGURATION_GLOB_ENDPOINT_OPTION: OnceCell<EndpointOptions> = OnceCell::new();
pub static CONFIGURATION_GLOB_NETWORK_OPTION: OnceCell<NetworkOptions> = OnceCell::new();
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Database {
    pub keep_history: u32,
}

#[derive(Serialize, Debug, Clone)]
pub struct Configuration {
    pub global: Global,
    pub database: Database,
    #[serde(deserialize_with = "deserialize_protocols")]
    pub protocols: HashMap<ProtocolName, HashMap<NetworkName, ProtocolsOpts>>,
    #[serde(skip)]
    pub enabled_proto_net: HashMap<ProtocolName, Vec<NetworkName>>,
}
/**
 * Deserialize configuration is used to be sure global configuration will be deserialized first
 * global configuration set some default values wich endpoint will reuse at their initialization
 */
impl<'de> Deserialize<'de> for Configuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        debug!("deserialize_configuration");
        let v: Value = Deserialize::deserialize(deserializer)?;

        let global = Global::deserialize(v.as_object().unwrap().get("global").unwrap()).unwrap();
        let database =
            Database::deserialize(v.as_object().unwrap().get("database").unwrap()).unwrap();
        let protocols =
            deserialize_protocols(v.as_object().unwrap().get("protocols").unwrap()).unwrap();
        let mut enabled_proto_net = HashMap::new();
        protocols.iter().for_each(|(proto, net)| {
            enabled_proto_net.insert(proto.clone(), net.keys().cloned().collect());
        });

        Ok(Configuration {
            global,
            database,
            protocols,
            enabled_proto_net,
        })
    }
}

fn deserialize_protocols<'de, D>(
    deserializer: D,
) -> Result<HashMap<ProtocolName, HashMap<NetworkName, ProtocolsOpts>>, D::Error>
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
                    debug!("Deserialize Bitcoin options {:?}", v);
                    let mut bitcoin_opts = BitcoinOpts::default();
                    for (endpoint_param_name, endpoint_obj) in v.as_object().unwrap() {
                        match endpoint_param_name.as_str() {
                            "network_options" => {
                                let network_opts =
                                    NetworkOptions::deserialize(endpoint_obj).unwrap();
                                bitcoin_opts.network_options = Some(network_opts);
                            }
                            "rpc" => {
                                let rpc_endpts: Vec<Endpoint> =
                                    serde_json::from_str(&endpoint_obj.to_string()).unwrap();
                                let bitcoin_nodes = rpc_endpts
                                    .iter()
                                    .map(|e| {
                                        let mut e = e.clone();
                                        e.network = k.clone();
                                        BitcoinNode::new(
                                            EndpointOptions::new(),
                                            crate::conf2::Network2::Mainnet,
                                        ) //FIXME : CHanged for test config2
                                    })
                                    .collect();
                                bitcoin_opts.rpc = Some(bitcoin_nodes);
                            }
                            _ => {
                                let mut endpoint = Endpoint::deserialize(endpoint_obj).unwrap();
                                endpoint.network = k.clone();
                                let endpoint_param_name =
                                    BitcoinAvailableEndpoints::from_str(endpoint_param_name);
                                match endpoint_param_name {
                                    Ok(BitcoinAvailableEndpoints::Blockstream) => {
                                        let blockstream = Blockstream::new(
                                            EndpointOptions::new(),
                                            crate::conf2::Network2::Mainnet,
                                        ); //FIXME : CHanged for test config2
                                        bitcoin_opts.blockstream = Some(blockstream);
                                    }
                                    Ok(BitcoinAvailableEndpoints::Blockcypher) => {
                                        let blockcypher = Blockcypher::new(
                                            EndpointOptions::new(),
                                            crate::conf2::Network2::Mainnet,
                                        ); //FIXME : CHanged for test config2
                                        bitcoin_opts.blockcypher = Some(blockcypher);
                                    }
                                    _ => {
                                        error!(
                                            "Unknown Bitcoin endpoint : {:?}",
                                            endpoint_param_name
                                        );
                                        panic!("Bitcoin endpoint configuration contain unknown option : {:?}", endpoint_param_name)
                                    }
                                }
                            }
                        }
                    }

                    map.entry(proto_k.clone())
                        .or_insert(HashMap::new())
                        .insert(k, ProtocolsOpts::Bitcoin(bitcoin_opts));
                }
                ProtocolName::Ethereum => {
                    debug!("Deserialize Ethereum options {:?}", v);
                    //FIXME: Should find way to don't repeat code for each proto but.. VOILA
                    let mut ethereum_opts = EthereumOpts::default();
                    for (endpoint_param_name, endpoint_obj) in v.as_object().unwrap() {
                        match endpoint_param_name.as_str() {
                            "network_options" => {
                                let network_opts =
                                    NetworkOptions::deserialize(endpoint_obj).unwrap();
                                info!("Ethereum network options : {:?}", network_opts);
                                ethereum_opts.network_options = Some(network_opts);
                            }
                            "rpc" => {
                                let rpc_endpts: Vec<Endpoint> =
                                    serde_json::from_str(&endpoint_obj.to_string()).unwrap();
                                let ethereum_nodes = rpc_endpts
                                    .iter()
                                    .map(|e| {
                                        let mut e = e.clone();
                                        e.network = k.clone();
                                        EthereumNode::new(
                                            EndpointOptions::new(),
                                            crate::conf2::Network2::Mainnet,
                                        )
                                    })
                                    .collect();
                                ethereum_opts.rpc = Some(ethereum_nodes);
                            }
                            _ => {
                                error!("Unknown Ethereum endpoint : {:?}", endpoint_param_name);
                                panic!(
                                    "Ethereum endpoint configuration contain unknown option : {:?}",
                                    endpoint_param_name
                                )
                            }
                        }
                    }
                    ethereum_opts.network_options = match ethereum_opts.network_options {
                        Some(e) => Some(e),
                        None => Some(NetworkOptions::default()),
                    };

                    map.entry(proto_k.clone())
                        .or_insert(HashMap::new())
                        .insert(k, ProtocolsOpts::Ethereum(ethereum_opts));
                }
                ProtocolName::Tezos => {
                    let endpoints = TezosEndpoints::deserialize(v).unwrap();
                    map.entry(proto_k.clone())
                        .or_insert(HashMap::new())
                        .insert(k, ProtocolsOpts::Tezos(endpoints));
                }
            }
        }
    }
    Ok(map)
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Global {
    #[serde(deserialize_with = "deserialize_global_endpoint_options")]
    pub endpoints: EndpointOptions,
    #[serde(deserialize_with = "deserialize_global_network_options")]
    pub networks_options: NetworkOptions,
    pub metrics: Metrics,
    pub server: Server,
}
// deserialize_global_endpoint_options
// Little trick to init global endpoint options, this Global will be used in the rest of deserialization process to init request client
fn deserialize_global_endpoint_options<'de, D>(deserializer: D) -> Result<EndpointOptions, D::Error>
where
    D: Deserializer<'de>,
{
    let endpoint_opt = EndpointOptions::deserialize(deserializer).unwrap();
    debug!("deserialize_global_endpoint_options: {:?}", endpoint_opt);
    CONFIGURATION_GLOB_ENDPOINT_OPTION
        .set(endpoint_opt.clone())
        .unwrap();
    debug!(
        "set CONFIGURATION_GLOB_ENDPOINT_OPTION: {:?}",
        CONFIGURATION_GLOB_ENDPOINT_OPTION.get().unwrap()
    );
    Ok(endpoint_opt)
}
// deserialize_global_network_options
// Little trick to init global network options, this Global will be used in the rest of deserialization process to init request client
fn deserialize_global_network_options<'de, D>(deserializer: D) -> Result<NetworkOptions, D::Error>
where
    D: Deserializer<'de>,
{
    let network_opt = NetworkOptions::deserialize(deserializer).unwrap();
    debug!("deserialize_global_network_options: {:?}", network_opt);
    CONFIGURATION_GLOB_NETWORK_OPTION
        .set(network_opt.clone())
        .unwrap();
    debug!(
        "set CONFIGURATION_GLOB_NETWORK_OPTION: {:?}",
        CONFIGURATION_GLOB_NETWORK_OPTION.get().unwrap()
    );
    Ok(network_opt)
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Metrics {
    pub port: u16,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Server {
    pub port: u16,
}
#[derive(Deserialize, Serialize, Debug, Clone, Eq, Hash, PartialEq)]
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
    InitState,
}

impl NetworkName {
    pub fn to_string(&self) -> String {
        match self {
            NetworkName::Mainnet => "mainnet".to_string(),
            NetworkName::Testnet => "testnet".to_string(),
            NetworkName::Goerli => "goerli".to_string(),
            NetworkName::Sepolia => "sepolia".to_string(),
            NetworkName::Ghostnet => "ghostnet".to_string(),
            NetworkName::InitState => "".to_string(),
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
            "" => Ok(NetworkName::InitState),
            _ => Err(format!("{} is not a valid network name", s)),
        }
    }
}
impl std::fmt::Display for NetworkName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            NetworkName::Mainnet => write!(f, "mainnet"),
            NetworkName::Testnet => write!(f, "testnet"),
            NetworkName::Goerli => write!(f, "goerli"),
            NetworkName::Sepolia => write!(f, "sepolia"),
            NetworkName::Ghostnet => write!(f, "ghostnet"),
            NetworkName::InitState => write!(f, ""),
        }
    }
}
/*
* get_enabled_protocol_network
* Return a HashMap of enabled protocol and network
*/
pub fn get_enabled_protocol_network() -> HashMap<ProtocolName, Vec<NetworkName>> {
    let config = CONFIGURATION.get().unwrap();
    config.enabled_proto_net.clone()
}

#[derive(Serialize, Debug, Clone, Eq, Hash, PartialEq)]
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

impl<'de> Deserialize<'de> for ProtocolName {
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolsOpts {
    Bitcoin(BitcoinOpts),
    Ethereum(EthereumOpts),
    Tezos(TezosEndpoints),
}

#[derive(Serialize, Debug, Clone)]
pub struct NetworkOptions {
    pub head_length: Option<u32>,
    pub tick_rate: u32,
}
impl<'de> Deserialize<'de> for NetworkOptions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut network_options = NetworkOptions::default();

        let map = serde_json::Map::deserialize(deserializer)?;
        error!("deserialize NetworkOptions {:?}", map);
        for (key, value) in map {
            match key.as_str() {
                "head_length" => {
                    network_options.head_length = Some(value.as_u64().unwrap() as u32);
                }
                "tick_rate" => {
                    network_options.tick_rate = value.as_u64().unwrap() as u32;
                }
                _ => {
                    return Err(serde::de::Error::custom(format!(
                        "Unknown network option: {}",
                        key
                    )));
                }
            }
        }
        info!("network_options: {:?}", network_options);

        Ok(network_options)
    }
}

impl Default for NetworkOptions {
    fn default() -> Self {
        trace!("default NetworkOptions");
        match CONFIGURATION_GLOB_NETWORK_OPTION.get() {
            Some(global_networks_opts) => {
                let global_networks_options = global_networks_opts.clone();
                NetworkOptions {
                    head_length: global_networks_options.head_length,
                    tick_rate: global_networks_options.tick_rate,
                }
            }
            None => NetworkOptions {
                head_length: Some(DEFAULT_HEAD_LENGTH),
                tick_rate: DEFAULT_TICK_RATE,
            },
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct BitcoinOpts {
    pub network_options: Option<NetworkOptions>,
    pub rpc: Option<Vec<BitcoinNode>>,
    pub blockstream: Option<Blockstream>,
    pub blockcypher: Option<Blockcypher>,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
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

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct EthereumOpts {
    pub network_options: Option<NetworkOptions>,
    pub rpc: Option<Vec<EthereumNode>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TezosEndpoints {
    pub rpc: Option<Vec<Endpoint>>,
    pub tzstats: Option<Endpoint>,
    pub tzkt: Option<Endpoint>,
}
#[derive(Serialize, Debug, Clone)]
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
impl Endpoint {
    pub fn test_new(url: &str, net: NetworkName) -> Self {
        let opt = EndpointOptions::test_new(url);
        Endpoint {
            last_request: 0,
            url: url.to_string(),
            network: net,
            reqwest: Some(ReqwestClient::new(opt)),
        }
    }
}
impl EndpointActions for Endpoint {
    fn set_last_request(&mut self) {
        self.last_request = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
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
pub trait EndpointActions {
    fn set_last_request(&mut self);
    fn available(&self) -> bool;
}
// Endpoint deserialization will substitute options field, to init reqwest client
impl<'de> Deserialize<'de> for Endpoint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v: Value = Deserialize::deserialize(deserializer)?;
        let url = v
            .get("url")
            .ok_or_else(|| serde::de::Error::custom("Missing url"))?
            .as_str()
            .ok_or_else(|| serde::de::Error::custom("Invalid url"))?
            .to_string();
        let options = v.get("options");
        error!("Options: {:?}", options);
        let options = match options {
            Some(options) => {
                let mut options = serde_json::from_value::<EndpointOptions>(options.clone())
                    .map_err(|e| serde::de::Error::custom(format!("Invalid options: {}", e)))?;
                options.init(Some(&url));
                options
            }
            None => {
                let mut options = EndpointOptions::default();
                options.init(Some(&url));
                options
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
// #[derive(Deserialize, Serialize, Debug, Clone)]
// pub struct EndpointOptions {
//     pub url: Option<String>,
//     pub retry: Option<u32>,
//     pub delay: Option<u32>,
//     pub rate: Option<u32>,
// }
impl EndpointOptions {
    pub fn new() -> Self {
        EndpointOptions {
            url: None,
            retry: None,
            delay: None,
            rate: None,
        }
    }
    pub fn init(&mut self, url: Option<&str>) {
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
    pub fn test_new(url: &str) -> Self {
        EndpointOptions {
            url: Some(url.to_string()),
            retry: Some(3),
            delay: Some(1),
            rate: Some(5),
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
            rate: global_endpoint_conf.rate,
        }
    }
}

pub const DEFAULT_SERVER_PORT: u32 = 8080;
pub const DEFAULT_METRICS_PORT: u16 = 8081;
pub const DEFAULT_HEAD_LENGTH: u32 = 5;
pub const DEFAULT_TICK_RATE: u32 = 5;
pub const DEFAULT_ENDPOINT_RETRY: u32 = 3;
pub const DEFAULT_ENDPOINT_DELAY: u32 = 1;
pub const DEFAULT_ENDPOINT_REQUEST_RATE: u32 = 5;
pub const DEFAULT_DATABASE_KEEP_HISTORY: u32 = 1000;

impl Configuration {
    pub fn new() -> Result<Self, ConfigError> {
        // TODO: config file should be overridable by env variables
        // TODO: config file should be overridable by cli args
        let builder = config::Config::builder()
            .set_default("global.server.port", DEFAULT_SERVER_PORT)?
            .set_default("global.metrics.port", DEFAULT_METRICS_PORT)?
            .set_default("global.networks_options.head_length", DEFAULT_HEAD_LENGTH)?
            .set_default("global.networks_options.tick_rate", DEFAULT_TICK_RATE)?
            .set_default("database.keep_history", DEFAULT_DATABASE_KEEP_HISTORY)?
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
