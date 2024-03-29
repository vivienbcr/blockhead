use crate::{
    endpoints::{
        bitcoin_node::BitcoinNode, blockcypher::Blockcypher, blockstream::Blockstream,
        ethereum_node::EthereumNode, polkadot_node::PolkadotNode, starknet_node::StarknetNode,
        subscan::Subscan, tezos_node::TezosNode, tzkt::Tzkt, tzstats::TzStats, ProviderActions,
    },
    requests::client::ReqwestClient,
};

use clap::{Parser, ValueEnum};
use config::{self, ConfigError, File};
use std::{
    io,
    sync::{PoisonError, RwLock},
};
use tokio::sync::mpsc;

use env_logger::Env;
use once_cell::sync::{Lazy, OnceCell};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{collections::HashMap, ffi::OsString, path::PathBuf};

pub static CONFIGURATION: Lazy<RwLock<Option<Configuration>>> = Lazy::new(|| RwLock::new(None));
pub static CONFIGURATION_PATH: OnceCell<PathBuf> = OnceCell::new();

type NetworkProvider = HashMap<Network, Vec<Provider>>;
type NetworkOptions = HashMap<Network, NetworkAppOptions>;
type ProtocolsNetworksOpts = HashMap<Protocol, NetworkOptions>;
type ProtocolsNetworksProviders = HashMap<Protocol, NetworkProvider>;
struct ProtoOptsProvider {
    pub proto_opts: ProtocolsNetworksOpts,
    pub proto_providers: ProtocolsNetworksProviders,
}

/**
 * Configuration is the main struct used to store all configuration
 */
#[derive(Debug, Clone)]
pub struct Configuration {
    pub global: Global,
    pub database: Database,
    pub proto_opts: ProtocolsNetworksOpts,
    pub proto_providers: ProtocolsNetworksProviders,
}
impl Configuration {
    pub fn get_network_options(
        &self,
        protocol: &Protocol,
        network: &Network,
    ) -> Option<&NetworkAppOptions> {
        self.proto_opts.get(protocol)?.get(network)
    }
}
pub fn get_configuration() -> Option<Configuration> {
    let config = CONFIGURATION.read();
    // if lock error
    match config {
        Ok(c) => c.clone(),
        Err(e) => panic!("Error while getting configuration: {}", e),
    }
}
pub fn set_configuration(
    conf: Option<Configuration>,
) -> Result<(), PoisonError<std::sync::RwLockWriteGuard<'static, Option<Configuration>>>> {
    let config: Result<
        std::sync::RwLockWriteGuard<Option<Configuration>>,
        PoisonError<std::sync::RwLockWriteGuard<Option<Configuration>>>,
    > = CONFIGURATION.write();
    match config {
        Ok(mut c) => {
            *c = conf;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub fn get_enabled_protocol_network() -> Option<HashMap<Protocol, Vec<Network>>> {
    let config = get_configuration().unwrap(); // FIXME
    let mut res = HashMap::new();
    for (proto, networks) in &config.proto_opts {
        let mut net_names = Vec::new();
        for net in networks {
            net_names.push(net.0.clone());
        }
        res.insert(*proto, net_names);
    }
    if res.is_empty() {
        return None;
    }
    Some(res)
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
        // protocol options, and providers come from same block in configuration file
        // they are deserialized together with in temp struct
        let proto_opts_provider = deserialize_proto_opts_provider(
            v.as_object().unwrap().get("protocols").unwrap(),
            &global,
        )
        .unwrap();

        Ok(Configuration {
            global,
            database,
            proto_opts: proto_opts_provider.proto_opts,
            proto_providers: proto_opts_provider.proto_providers,
        })
    }
}
fn deserialize_proto_opts_provider<'de, D>(
    deserializer: D,
    global: &Global,
) -> Result<ProtoOptsProvider, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Value = Deserialize::deserialize(deserializer)?;
    /*
     * proto_opts store network options for each protocol
     */
    let mut proto_opts: ProtocolsNetworksOpts = HashMap::new();
    /*
     * proto_providers store providers for each protocol
     */
    let mut proto_providers: ProtocolsNetworksProviders = HashMap::new();
    /*
     * Deserialize protocols
     * For each protocol we deserialize network options and providers
     */
    v.as_object()
        .unwrap()
        .iter()
        .for_each(|(proto, proto_config)| {
            debug!("Deserialize protocol {}", proto);

            let mut net_providers: NetworkProvider = HashMap::new();
            let protocol = Protocol::from(proto.clone());
            let protocol = match protocol {
                Some(p) => p,
                _ => {
                    panic!("Unkonwn protocol: {} found in configuration file", proto)
                }
            };
            let mut net_opts: NetworkOptions = HashMap::new();
            /*
             * Deserialize networks
             * For each network we deserialize network options and providers
             */
            let s: Value = serde_json::from_str(&proto_config.to_string()).unwrap();
            s.as_object().unwrap().iter().for_each(|(net, opts)| {
                debug!("Deserialize network {}", net);
                let network = net.clone();
                let o: Value = serde_json::from_str(&opts.to_string()).unwrap();
                /*
                 * Deserialize Network options
                 */
                let network_option_field = o.as_object().unwrap().get("network_options");
                match network_option_field {
                    Some(opt) => {
                        let mut net_opt = global.networks_options.clone();
                        net_opt
                            .from_network_option_file(
                                &NetworkAppOptionsConfigF::deserialize(opt).unwrap(),
                            )
                            .unwrap();

                        net_opts.insert(network.clone(), net_opt);
                    }
                    None => {
                        net_opts.insert(network.clone(), global.networks_options.clone());
                    }
                }
                /*
                 * Deserialize providers
                 */
                let mut providers = Vec::new();
                let mut alias_list = Vec::new();
                o.as_object().unwrap().iter().for_each(|(provider, opt)| {
                    let endpoints_options: Value = serde_json::from_str(&opt.to_string()).unwrap();
                    match provider.as_str() {
                        "network_options" => {}
                        // Rpc is vec of provider declaration,
                        "rpc" => {
                            debug!("Found rpc {}", provider);
                            endpoints_options
                                .as_array()
                                .unwrap_or(&Vec::new())
                                .iter()
                                .for_each(|endpoint| {
                                    // merge endpoint options with global endpoint options
                                    let provider_config_f =
                                        ProviderConfigF::deserialize(endpoint).unwrap();

                                    let endpoint_opts = EndpointOptions::from_provider_config_f(
                                        provider_config_f,
                                        &global.endpoints,
                                    );

                                    debug!("endpoint_opt: {:?}", endpoint_opts);
                                    // Alias should be unique per protocol / network
                                    if !alias_list.contains(&endpoint_opts.alias) {
                                        if endpoint_opts.alias != default_alias(){
                                            alias_list.push(endpoint_opts.alias.clone());
                                        }
                                    } else {
                                        panic!("Found duplicated alias {} in config file at protocol: {} network: {} ", endpoint_opts.alias, protocol, network);
                                    }
                                    let rpc_provider = Provider::from_str(
                                        &format!("{}_node", protocol),
                                        endpoint_opts,
                                        &network,
                                    );
                                    providers.push(rpc_provider);
                                })
                        }
                        // Str is a provider declaration
                        str => {
                            debug!("Found provider {}", str);
                            if Provider::is_available(str) {
                                let provider_config_f = ProviderConfigF::deserialize(opt).unwrap();
                                let endpoint_opts = EndpointOptions::from_provider_config_f(
                                    provider_config_f,
                                    &global.endpoints,
                                );
                                let provider = Provider::from_str(str, endpoint_opts, &network);
                                providers.push(provider);
                            } else {
                                panic!(
                                    "Provider {} is not available for {:?} {:?} ",
                                    str, protocol, network
                                );
                            }
                        }
                    }
                });
                debug!(
                    "Protocol: {} Network: {} Providers: {}",
                    protocol.to_string(),
                    network,
                    providers.len()
                );
                net_providers.insert(network, providers);
            });
            // After deserialization, if user didn't specify network options we use global options
            proto_opts.insert(protocol, net_opts);
            proto_providers.insert(protocol, net_providers);
        });
    Ok(ProtoOptsProvider {
        proto_opts,
        proto_providers,
    })
}
/**
 * Global configuration is used to store application configuration
 */
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Global {
    #[serde(rename(deserialize = "options"))]
    #[serde(default)]
    pub endpoints: EndpointOptions,
    pub networks_options: NetworkAppOptions,
    pub metrics: Metrics,
    pub server: Server,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Metrics {
    pub port: u16,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Server {
    pub port: u16,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Database {
    #[serde(default = "default_database_keep_history")]
    pub keep_history: u32,
    #[serde(default = "default_database_path")]
    #[serde(deserialize_with = "deserialize_path")]
    pub path: PathBuf,
}

fn deserialize_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    Ok(PathBuf::from(s))
}

#[derive(Debug, Clone)]
pub enum Provider {
    Blockstream(Blockstream),
    Blockcypher(Blockcypher),
    BitcoinNode(BitcoinNode),
    EthereumNode(EthereumNode),
    EwfNode(EthereumNode),
    TezosNode(TezosNode),
    PolkadotNode(PolkadotNode),
    Tzkt(Tzkt),
    TzStats(TzStats),
    Subscan(Subscan),
    MoonbeamNode(EthereumNode),
    StarknetNode(StarknetNode),
    AvalancheNode(EthereumNode),
    PolygonNode(EthereumNode),
    None,
}
#[cfg(test)]
pub fn get_bitcoin_nodes(providers: &Vec<Provider>) -> Vec<&BitcoinNode> {
    let mut bitcoin_nodes = Vec::new();
    for provider in providers {
        match provider {
            Provider::BitcoinNode(node) => bitcoin_nodes.push(node),
            _ => (),
        }
    }
    bitcoin_nodes
}
#[cfg(test)]
pub fn get_blockstream(providers: &Vec<Provider>) -> Vec<&Blockstream> {
    let mut blockstream = Vec::new();
    for provider in providers {
        match provider {
            Provider::Blockstream(node) => blockstream.push(node),
            _ => (),
        }
    }
    blockstream
}
#[cfg(test)]
pub fn get_blockcypher(providers: &Vec<Provider>) -> Vec<&Blockcypher> {
    let mut blockcypher = Vec::new();
    for provider in providers {
        match provider {
            Provider::Blockcypher(node) => blockcypher.push(node),
            _ => (),
        }
    }
    blockcypher
}
#[cfg(test)]
pub fn get_ethereum_nodes(providers: &Vec<Provider>) -> Vec<&EthereumNode> {
    let mut ethereum_nodes = Vec::new();
    for provider in providers {
        match provider {
            Provider::EthereumNode(node) => ethereum_nodes.push(node),
            _ => (),
        }
    }
    ethereum_nodes
}

impl Provider {
    pub fn from_str(provider: &str, endpoint_opt: EndpointOptions, network: &Network) -> Provider {
        let n = network.to_owned();
        match provider {
            "blockstream" => {
                Provider::Blockstream(Blockstream::new(endpoint_opt, Protocol::Bitcoin, n))
            }
            "blockcypher" => {
                Provider::Blockcypher(Blockcypher::new(endpoint_opt, Protocol::Bitcoin, n))
            }
            "bitcoin_node" => {
                Provider::BitcoinNode(BitcoinNode::new(endpoint_opt, Protocol::Bitcoin, n))
            }
            "ethereum_node" => {
                Provider::EthereumNode(EthereumNode::new(endpoint_opt, Protocol::Ethereum, n))
            }
            "ewf_node" => Provider::EwfNode(EthereumNode::new(endpoint_opt, Protocol::Ewf, n)),
            "polygon_node" => {
                Provider::PolygonNode(EthereumNode::new(endpoint_opt, Protocol::Polygon, n))
            }
            "tezos_node" => Provider::TezosNode(TezosNode::new(endpoint_opt, Protocol::Tezos, n)),
            "tzkt" => Provider::Tzkt(Tzkt::new(endpoint_opt, Protocol::Tezos, n)),
            "tzstats" => Provider::TzStats(TzStats::new(endpoint_opt, Protocol::Tezos, n)),
            "polkadot_node" => {
                Provider::PolkadotNode(PolkadotNode::new(endpoint_opt, Protocol::Polkadot, n))
            }
            "subscan" => Provider::Subscan(Subscan::new(endpoint_opt, Protocol::Polkadot, n)),
            "moonbeam_node" => {
                Provider::MoonbeamNode(EthereumNode::new(endpoint_opt, Protocol::Moonbeam, n))
            }
            "starknet_node" => {
                Provider::StarknetNode(StarknetNode::new(endpoint_opt, Protocol::Starknet, n))
            }
            "avalanche_node" => {
                let endpoint_opt = EndpointOptions {
                    url: Some(format!("{}{}", endpoint_opt.url.unwrap(), "/ext/bc/C/rpc")),
                    ..endpoint_opt
                };
                Provider::AvalancheNode(EthereumNode::new(endpoint_opt, Protocol::Avalanche, n))
            }
            _ => Provider::None,
        }
    }
    pub fn as_mut_provider_actions(&mut self) -> Option<&mut dyn ProviderActions> {
        match self {
            Provider::Blockstream(provider) => Some(provider),
            Provider::Blockcypher(provider) => Some(provider),
            Provider::BitcoinNode(provider) => Some(provider),
            Provider::EthereumNode(provider) => Some(provider),
            Provider::EwfNode(provider) => Some(provider),
            Provider::PolygonNode(provider) => Some(provider),
            Provider::TezosNode(provider) => Some(provider),
            Provider::Tzkt(provider) => Some(provider),
            Provider::TzStats(provider) => Some(provider),
            Provider::PolkadotNode(provider) => Some(provider),
            Provider::Subscan(provider) => Some(provider),
            Provider::MoonbeamNode(provider) => Some(provider),
            Provider::StarknetNode(provider) => Some(provider),
            Provider::AvalancheNode(provider) => Some(provider),
            _ => None,
        }
    }
    pub fn is_available(provider: &str) -> bool {
        matches!(
            provider,
            "blockstream"
                | "blockcypher"
                | "bitcoin_node"
                | "ethereum_node"
                | "ewf_node"
                | "polygon_node"
                | "tezos_node"
                | "tzkt"
                | "tzstats"
                | "polkadot_node"
                | "subscan"
                | "moonbeam_node"
                | "starknet_node"
                | "avalanche_node"
        )
    }
}

#[derive(Debug, Clone)]
pub enum EthereumProviders {
    Rpc(Vec<EthereumNode>),
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NetworkAppOptionsConfigF {
    pub head_length: Option<u32>,
    pub tick_rate: Option<u32>,
}
/**
 * Network options is used to define network specific options
 * With this you can fine tune the network scraping params on your needs
 */
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NetworkAppOptions {
    #[serde(default = "default_head_length")]
    pub head_length: u32,
    #[serde(default = "default_tick_rate")]
    pub tick_rate: u32,
}
impl NetworkAppOptions {
    pub fn from_network_option_file(
        &mut self,
        network_option_file: &NetworkAppOptionsConfigF,
    ) -> Result<(), ConfigError> {
        if let Some(head_length) = network_option_file.head_length {
            self.head_length = head_length;
        }
        if let Some(tick_rate) = network_option_file.tick_rate {
            self.tick_rate = tick_rate;
        }
        Ok(())
    }
}
/**
 * Endpoint Options Config is config file structure
 */
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProviderOptsConfigF {
    pub retry: Option<u32>,
    pub delay: Option<u32>,
    pub rate: Option<u32>,
    #[serde(deserialize_with = "deserialize_string_hashmap")]
    #[serde(default = "default_headers")]
    pub headers: Option<HashMap<String, String>>,
    pub basic_auth: Option<BasicAuth>,
    #[serde(default = "default_alias")]
    pub alias: String,
}
fn default_headers() -> Option<HashMap<String, String>> {
    None
}
// deserialize_string_hashmap is used to deserialize headers from config file, any value should be converted to string
fn deserialize_string_hashmap<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, serde_json::Value> = HashMap::deserialize(deserializer)?;
    if map.is_empty() {
        return Ok(None);
    }
    let mut string_map: HashMap<String, String> = HashMap::new();
    for (key, value) in map {
        let value = match value {
            serde_json::Value::Number(n) => {
                if n.is_i64() {
                    n.as_i64().unwrap().to_string()
                } else if n.is_u64() {
                    n.as_u64().unwrap().to_string()
                } else {
                    n.as_f64().unwrap().to_string()
                }
            }
            serde_json::Value::String(s) => s,
            _ => value.to_string(),
        };

        string_map.insert(key, value);
    }
    Ok(Some(string_map))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProviderConfigF {
    pub url: Option<String>,
    pub options: Option<ProviderOptsConfigF>,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BasicAuth {
    #[serde(default = "default_basic_auth_username")]
    pub username: String,
    pub password: String,
}
pub fn default_basic_auth_username() -> String {
    String::from("")
}

/**
 * Endpoint options is used to define reqwest client options
 * Don't reuse EndpointOptionsConfigF to avoid Option<Option<T>> everywhere
 * Endpoint options if not defined in config file will be set to default values
 */
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EndpointOptions {
    pub url: Option<String>,
    #[serde(default = "default_endpoint_retry")]
    pub retry: u32,
    #[serde(default = "default_endpoint_delay")]
    pub delay: u32,
    #[serde(default = "default_endpoint_request_rate")]
    pub rate: u32,
    #[serde(default = "default_endpoint_request_timeout")]
    pub timeout: u32,
    pub headers: Option<HashMap<String, String>>,
    pub basic_auth: Option<BasicAuth>,
    #[serde(default = "default_alias")]
    pub alias: String,
}
impl Default for EndpointOptions {
    fn default() -> Self {
        let global = get_configuration();
        match global {
            Some(g) => g.global.endpoints,
            None => EndpointOptions {
                url: None,
                retry: default_endpoint_retry(),
                delay: default_endpoint_delay(),
                rate: default_endpoint_request_rate(),
                timeout: default_endpoint_request_timeout(),
                headers: None,
                basic_auth: None,
                alias: "".to_string(),
            },
        }
    }
}

impl EndpointOptions {
    /**
     * from_provider_config_f is used to create EndpointOptions from ProviderConfigF
     * Take global options and override values specified in ProviderConfigF
     * Global is required because Default trait will not consider global config override
     */
    pub fn from_provider_config_f(
        provider: ProviderConfigF,
        global: &EndpointOptions,
    ) -> EndpointOptions {
        let mut endpoint_opt = global.clone();
        if let Some(url) = provider.url {
            endpoint_opt.url = Some(url);
        }
        // endpoint_opt.alias = get_base_url(endpoint_opt.url.clone().unwrap().as_str());

        if let Some(options) = provider.options {
            if let Some(retry) = options.retry {
                endpoint_opt.retry = retry;
            }
            if let Some(delay) = options.delay {
                endpoint_opt.delay = delay;
            }
            if let Some(rate) = options.rate {
                endpoint_opt.rate = rate;
            }
            if let Some(headers) = options.headers {
                endpoint_opt.headers = Some(headers);
            }
            if let Some(basic_auth) = options.basic_auth {
                endpoint_opt.basic_auth = Some(basic_auth);
            }
            if options.alias != default_alias() {
                endpoint_opt.alias = options.alias;
            }
        }
        endpoint_opt
    }
    #[cfg(test)]
    pub fn test_new(
        url: &str,
        headers: Option<HashMap<String, String>>,
        basic_auth: Option<BasicAuth>,
    ) -> Self {
        EndpointOptions {
            url: Some(url.to_string()),
            retry: 10,
            delay: 1,
            rate: 0,
            timeout: default_endpoint_request_timeout(),
            headers,
            basic_auth,
            alias: DEFAULT_ALIAS.to_string(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, Hash, PartialEq, Copy)]
pub enum Protocol {
    #[serde(rename = "bitcoin")]
    Bitcoin,
    #[serde(rename = "ethereum")]
    Ethereum,
    #[serde(rename = "ewf")]
    Ewf,
    #[serde(rename = "polygon")]
    Polygon,
    #[serde(rename = "tezos")]
    Tezos,
    #[serde(rename = "polkadot")]
    Polkadot,
    #[serde(rename = "moonbeam")]
    Moonbeam,
    #[serde(rename = "starknet")]
    Starknet,
    #[serde(rename = "avalanche")]
    Avalanche,
    #[serde(rename = "None")]
    None,
}

impl Protocol {
    fn from(s: String) -> Option<Self> {
        match s.as_str() {
            "bitcoin" => Some(Protocol::Bitcoin),
            "ethereum" => Some(Protocol::Ethereum),
            "ewf" => Some(Protocol::Ewf),
            "polygon" => Some(Protocol::Polygon),
            "tezos" => Some(Protocol::Tezos),
            "polkadot" => Some(Protocol::Polkadot),
            "moonbeam" => Some(Protocol::Moonbeam),
            "starknet" => Some(Protocol::Starknet),
            "avalanche" => Some(Protocol::Avalanche),
            _ => None,
        }
    }
}
impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Protocol::Bitcoin => "bitcoin",
            Protocol::Ethereum => "ethereum",
            Protocol::Ewf => "ewf",
            Protocol::Polygon => "polygon",
            Protocol::Tezos => "tezos",
            Protocol::Polkadot => "polkadot",
            Protocol::Moonbeam => "moonbeam",
            Protocol::Starknet => "starknet",
            Protocol::Avalanche => "avalanche",
            Protocol::None => "None",
        };
        write!(f, "{}", s)
    }
}

pub type Network = String;

#[derive(Serialize, Debug, Clone)]
pub struct Endpoint {
    pub url: String,
    #[serde(skip)]
    pub reqwest: ReqwestClient,
    #[serde(skip)]
    pub network: Network,
    #[serde(skip)]
    pub protocol: Protocol,
    #[serde(skip)]
    pub last_request: u64,
}
impl Endpoint {
    #[cfg(test)]
    pub fn test_new(
        url: &str,
        proto: Protocol,
        net: Network,
        headers: Option<HashMap<String, String>>,
        basic_auth: Option<BasicAuth>,
    ) -> Self {
        let opt = EndpointOptions::test_new(url, headers, basic_auth);
        Endpoint {
            last_request: 0,
            url: url.to_string(),
            protocol: proto,
            network: net,
            reqwest: ReqwestClient::new(opt),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Default)]
pub enum LogLevel {
    #[default]
    Info,
    Debug,
    Trace,
}

impl std::str::FromStr for LogLevel {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "info" => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            "trace" => Ok(LogLevel::Trace),
            _ => Err(()),
        }
    }
}
// implement trait `Display` for type `conf::LogLevel` instead
impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        };
        write!(f, "{}", s)
    }
}

#[derive(Parser)]
struct Args {
    #[arg(short, long, value_enum, default_value = "info")]
    log_level: Option<LogLevel>,
    #[arg(short, long)]
    db_path: Option<PathBuf>,
    #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
    config: Option<PathBuf>,
}

pub const DEFAULT_SERVER_PORT: u16 = 8080;
pub fn default_server_port() -> u16 {
    DEFAULT_SERVER_PORT
}
pub const DEFAULT_METRICS_PORT: u16 = 8081;
pub fn default_metrics_port() -> u16 {
    DEFAULT_SERVER_PORT
}
pub const DEFAULT_HEAD_LENGTH: u32 = 5;
pub fn default_head_length() -> u32 {
    DEFAULT_HEAD_LENGTH
}
pub const DEFAULT_TICK_RATE: u32 = 5;
pub fn default_tick_rate() -> u32 {
    DEFAULT_TICK_RATE
}
pub const DEFAULT_ENDPOINT_RETRY: u32 = 3;
fn default_endpoint_retry() -> u32 {
    DEFAULT_ENDPOINT_RETRY
}
pub const DEFAULT_ENDPOINT_DELAY: u32 = 1;
fn default_endpoint_delay() -> u32 {
    DEFAULT_ENDPOINT_DELAY
}
pub const DEFAULT_ENDPOINT_REQUEST_RATE: u32 = 5;
fn default_endpoint_request_rate() -> u32 {
    DEFAULT_ENDPOINT_REQUEST_RATE
}
pub const DEFAULT_ENDPOINT_REQUEST_TIMEOUT: u32 = 10;
fn default_endpoint_request_timeout() -> u32 {
    DEFAULT_ENDPOINT_REQUEST_TIMEOUT
}
pub const DEFAULT_DATABASE_KEEP_HISTORY: u32 = 1000;
fn default_database_keep_history() -> u32 {
    DEFAULT_DATABASE_KEEP_HISTORY
}
pub const DEFAULT_ALIAS: &str = "";
fn default_alias() -> String {
    DEFAULT_ALIAS.to_string()
}

pub const DEFAULT_CONFIG_PATH: &str = "config.yaml";

pub const DEFAULT_DATABASE_PATH: &str = "blockhead.db";
fn default_database_path() -> PathBuf {
    PathBuf::from(DEFAULT_DATABASE_PATH)
}

pub const DEFAULT_LOG_LEVEL: &str = "info";

impl Configuration {
    pub fn new(
        file: Option<&str>,
        args: Option<Vec<OsString>>,
        init: bool,
    ) -> Result<Self, ConfigError> {
        let args = match args {
            Some(args) => Args::parse_from(args),
            None => Args::parse(),
        };
        let conf_path: PathBuf = match file {
            Some(path) => PathBuf::from(path),
            None => args.config.unwrap_or(PathBuf::from(DEFAULT_CONFIG_PATH)),
        };
        // Configuration path is set only once
        if CONFIGURATION_PATH.get().is_none() && init {
            CONFIGURATION_PATH.set(conf_path.clone()).unwrap();
        }

        let builder = config::Config::builder()
            .set_default("global.server.port", DEFAULT_SERVER_PORT)?
            .set_default("global.metrics.port", DEFAULT_METRICS_PORT)?
            .set_default("global.networks_options.head_length", DEFAULT_HEAD_LENGTH)?
            .set_default("global.networks_options.tick_rate", DEFAULT_TICK_RATE)?
            .set_default("database.keep_history", DEFAULT_DATABASE_KEEP_HISTORY)?
            .set_default("global.endpoints.retry", DEFAULT_ENDPOINT_RETRY)?
            .set_default("global.endpoints.delay", DEFAULT_ENDPOINT_DELAY)?
            .set_default("global.endpoints.rate", DEFAULT_ENDPOINT_REQUEST_RATE)?
            .set_default("global.endpoint.alias", DEFAULT_ALIAS)?
            .add_source(File::from(conf_path))
            .build()?;

        let config: Result<Configuration, ConfigError> = builder.try_deserialize();
        let mut config = match config {
            Ok(c) => c,
            Err(e) => return Err(e),
        };

        if let Some(p) = args.db_path {
            config.database.path = p
        }

        if init {
            set_configuration(Some(config.clone())).unwrap();
        }

        Ok(config)
    }
}

pub fn init_logger(args: Option<Vec<OsString>>) {
    let args = match args {
        Some(args) => Args::parse_from(args),
        None => Args::parse(),
    };
    let log_level = match args.log_level {
        Some(l) => l,
        None => LogLevel::default(),
    };

    let env = Env::default().default_filter_or(format!("blockhead={}", log_level));
    env_logger::init_from_env(env);
}

use futures::{
    channel::mpsc::{channel, Receiver},
    SinkExt, StreamExt,
};
use notify::{
    event::{AccessKind, AccessMode},
    Event, RecommendedWatcher, RecursiveMode, Watcher,
};

fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let watcher = notify::recommended_watcher(move |res| {
        futures::executor::block_on(async {
            tx.send(res).await.unwrap();
        })
    })?;

    Ok((watcher, rx))
}

pub async fn watch_configuration_change(tx: mpsc::Sender<bool>) -> Result<(), io::Error> {
    let (mut watcher, mut rx) = async_watcher().unwrap();

    let path = CONFIGURATION_PATH.get().unwrap();
    watcher.watch(path, RecursiveMode::NonRecursive).unwrap();

    while let Some(res) = rx.next().await {
        match res {
            Ok(event) => {
                println!("raw event: {:?}", event);
                match event {
                    Event {
                        kind: notify::event::EventKind::Access(AccessKind::Close(AccessMode::Write)),
                        ..
                    } => {
                        Configuration::new(None, None, true).unwrap();
                        tx.send(true).await.unwrap();
                    }
                    _ => continue,
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{conf::*, tests};
    use std::ffi::OsString;

    #[test]
    // test_config_endpoint

    fn conf_struct_endpoint_options() {
        let endpoint = EndpointOptions::default();
        assert_eq!(endpoint.url, None, "url should be empty");
        assert_eq!(
            endpoint.retry, DEFAULT_ENDPOINT_RETRY,
            "retry should match with default value"
        );
        assert_eq!(
            endpoint.delay, DEFAULT_ENDPOINT_DELAY,
            "delay should match with default value"
        );
        assert_eq!(
            endpoint.rate, DEFAULT_ENDPOINT_REQUEST_RATE,
            "rate should match with default value"
        );
        assert_eq!(
            endpoint.headers, None,
            "headers should match with default value"
        );
        assert!(
            endpoint.basic_auth.is_none(),
            "basic_auth should match with default value"
        );

        let headers: HashMap<String, String> =
            HashMap::from([("X-API-KEY".to_string(), "some_key".to_string())]);
        let basic_auth = BasicAuth {
            username: "user".to_string(),
            password: "pass".to_string(),
        };

        let provider_options_f = ProviderOptsConfigF {
            retry: Some(40),
            delay: None,
            rate: Some(60),
            headers: Some(headers),
            basic_auth: Some(basic_auth),
            alias: default_alias(),
        };

        let provider_config_f = ProviderConfigF {
            options: Some(provider_options_f),
            url: Some("http://localhost:8080".to_string()),
        };

        let merge = EndpointOptions::from_provider_config_f(provider_config_f, &endpoint);
        assert_eq!(
            merge.url,
            Some("http://localhost:8080".to_string()),
            "url should be set"
        );
        assert_eq!(merge.retry, 40, "retry should match with overriden value");
        assert_eq!(
            merge.delay, DEFAULT_ENDPOINT_DELAY,
            "delay should not change"
        );
        assert_eq!(merge.rate, 60, "rate should match with overriden value");
        let merged_header = merge.headers.unwrap();
        assert_eq!(merged_header.contains_key("X-API-KEY"), true);
        assert_eq!(
            merged_header.get("X-API-KEY"),
            Some(&"some_key".to_string())
        );
        let merged_basic_auth = merge.basic_auth.unwrap();
        assert_eq!(
            merged_basic_auth.username,
            "user".to_string(),
            "username should match with overriden value"
        );
        assert_eq!(
            merged_basic_auth.password,
            "pass".to_string(),
            "password should match with overriden value"
        );
    }

    #[test]
    // test_config_full tests the full configuration file with overwrites all default values
    fn conf_full() {
        tests::setup();
        // should override os params for tests
        let args = vec![OsString::from("blockhead")];
        let config =
            Configuration::new(Some("./tests/full_config.yaml"), Some(args), false).unwrap();

        // Test network_options
        assert_eq!(
            config.global.networks_options.head_length, 1,
            "head_length should be set to 1"
        );
        assert_eq!(
            config.global.networks_options.tick_rate, 2,
            "tick_rate should be set to 2"
        );
        // Test endpoints
        assert_eq!(
            config.global.endpoints.retry, 33,
            "retry should be set to 33"
        );
        assert_eq!(
            config.global.endpoints.delay, 44,
            "delay should be set to 44"
        );
        assert_eq!(config.global.endpoints.rate, 55, "rate should be set to 55");
        assert_eq!(config.global.endpoints.url, None, "url should be empty");
        // test api server options
        assert_eq!(config.global.server.port, 6, "port should be set to 6");
        assert_eq!(
            config.global.metrics.port, 7,
            "metrics port should be set to 7"
        );

        // test database options
        assert_eq!(
            config.database.keep_history, 88,
            "keep_history should be set to 88"
        );

        assert_eq!(
            config.database.path,
            PathBuf::from("/some/path/file.db"),
            "db path should be set to /some/path/file.db"
        );

        // Test bitcoin provider
        let bitcoin_net_provider = config.proto_providers.get(&Protocol::Bitcoin).unwrap();
        let bitcoin_mainnet_providers = bitcoin_net_provider.get("mainnet").unwrap();
        assert_eq!(
            bitcoin_mainnet_providers.len(),
            4,
            "should have 4 provider for bitcoin mainnet"
        );
        let bitcoin_mainnet_rpc_urls = vec![
            "https://rpc-bitcoin-mainnet-1.com",
            "https://rpc-bitcoin-mainnet-2.com",
        ];
        let bitcoin_mainnet_rpc_providers = get_bitcoin_nodes(bitcoin_mainnet_providers);
        // Test first bitcoin mainnet rpc url
        let b = bitcoin_mainnet_rpc_providers
            .iter()
            .find(|x| x.endpoint.url == bitcoin_mainnet_rpc_urls[0]);
        assert_eq!(
            b.is_some(),
            true,
            "First Bitcoin mainnet rpc url should be set"
        );
        let e = b.unwrap().endpoint.clone();
        let e = e.reqwest;
        assert_eq!(
            e.config.rate, config.global.endpoints.rate,
            "First Bitcoin mainnet rpc url should be set"
        );
        assert_eq!(
            e.config.retry, config.global.endpoints.retry,
            "First Bitcoin mainnet rpc url should be set"
        );
        assert_eq!(
            e.config.delay, config.global.endpoints.delay,
            "First Bitcoin mainnet rpc url should be set"
        );
        let merged_header = e.config.headers.unwrap();
        assert_eq!(merged_header.contains_key("X-API-Key"), true);
        assert_eq!(merged_header.get("X-API-Key"), Some(&"10".to_string()));
        assert_eq!(merged_header.contains_key("ANOTHER-NUM-HEADER"), true);
        assert_eq!(
            merged_header.get("ANOTHER-NUM-HEADER"),
            Some(&"11".to_string())
        );

        // Test second bitcoin mainnet rpc url with overriden values
        let b = bitcoin_mainnet_rpc_providers
            .iter()
            .find(|x| x.endpoint.url == bitcoin_mainnet_rpc_urls[1]);
        assert_eq!(
            b.is_some(),
            true,
            "First Bitcoin mainnet rpc url should be set"
        );

        let e = b.unwrap().endpoint.clone();
        let e = e.reqwest;
        assert_eq!(
            e.config.rate, 15,
            "First Bitcoin mainnet rpc url should be set"
        );
        assert_eq!(
            e.config.retry, 13,
            "First Bitcoin mainnet rpc url should be set"
        );
        assert_eq!(
            e.config.delay, 14,
            "First Bitcoin mainnet rpc url should be set"
        );

        let basic_auth = e.config.basic_auth.unwrap();
        assert_eq!(basic_auth.username, "user".to_string());
        assert_eq!(basic_auth.password, "pass".to_string());
        // Test bitcoin mainnet blockstream
        let bitcoin_mainnet_blockstream_url = "https://sample-url-3.com";
        let bitcoin_mainnet_blockstream_providers = get_blockstream(bitcoin_mainnet_providers);
        let b = bitcoin_mainnet_blockstream_providers
            .iter()
            .find(|x| x.endpoint.url == bitcoin_mainnet_blockstream_url);
        assert_eq!(
            b.is_some(),
            true,
            "Bitcoin mainnet blockstream url should be set"
        );
        let e = b.unwrap().endpoint.clone();
        let e = e.reqwest;
        assert_eq!(
            e.config.rate, 16,
            "Bitcoin mainnet blockstream url should be set"
        );
        assert_eq!(
            e.config.retry, 17,
            "Bitcoin mainnet blockstream url should be set"
        );
        assert_eq!(
            e.config.delay, 18,
            "Bitcoin mainnet blockstream url should be set"
        );
        // Test bitcoin blockcypher
        let bitcoin_mainnet_blockcypher_url = "https://sample-url-4.com";
        let bitcoin_mainnet_blockcypher_providers = get_blockcypher(bitcoin_mainnet_providers);
        let b = bitcoin_mainnet_blockcypher_providers
            .iter()
            .find(|x| x.endpoint.url == bitcoin_mainnet_blockcypher_url);
        assert_eq!(
            b.is_some(),
            true,
            "Bitcoin mainnet blockcypher url should be set"
        );
        let e = b.unwrap().endpoint.clone();
        let e = e.reqwest;
        assert_eq!(
            e.config.rate, 19,
            "Bitcoin mainnet blockcypher url should be set"
        );
        assert_eq!(
            e.config.retry, 20,
            "Bitcoin mainnet blockcypher url should be set"
        );
        assert_eq!(
            e.config.delay, 21,
            "Bitcoin mainnet blockcypher url should be set"
        );
        let bitcoin_network_options = config.proto_opts.get(&Protocol::Bitcoin).unwrap();
        let bitcoin_mainnet_network_options = bitcoin_network_options.get("mainnet").unwrap();
        assert_eq!(
            bitcoin_mainnet_network_options.head_length, 9,
            "Bitcoin mainnet head_length should be set to 9"
        );
        // Test ethereum provider
        let ethereum_net_provider = config.proto_providers.get(&Protocol::Ethereum).unwrap();
        let ethereum_mainnet_providers = ethereum_net_provider.get("mainnet").unwrap();
        assert_eq!(
            ethereum_mainnet_providers.len(),
            1,
            "should have 1 provider for ethereum mainnet"
        );
        let ethereum_mainnet_rpc_urls = vec!["https://rpc-ethereum-5.com"];
        let ethereum_mainnet_rpc_providers = get_ethereum_nodes(ethereum_mainnet_providers);
        // Test first ethereum mainnet rpc url
        let b = ethereum_mainnet_rpc_providers
            .iter()
            .find(|x| x.endpoint.url == ethereum_mainnet_rpc_urls[0]);
        assert_eq!(
            b.is_some(),
            true,
            "First Ethereum mainnet rpc url should be set"
        );
        let e = b.unwrap().endpoint.clone();
        let e = e.reqwest;
        assert_eq!(
            e.config.retry, 22,
            "First Ethereum mainnet retry should be equal to 22"
        );
        assert_eq!(
            e.config.delay, 23,
            "First Ethereum mainnet delay should be equal to 23"
        );
        assert_eq!(
            e.config.rate, 24,
            "First Ethereum mainnet rate should be equal to 24"
        );
        let ethereum_network_options = config.proto_opts.get(&Protocol::Ethereum).unwrap();
        let ethereum_mainnet_network_options = ethereum_network_options.get("mainnet").unwrap();
        assert_eq!(
            ethereum_mainnet_network_options.head_length,
            config.global.networks_options.head_length,
            "Ethereum mainnet head_length should be eq to global head_length"
        );
        assert_eq!(
            ethereum_mainnet_network_options.tick_rate, config.global.networks_options.tick_rate,
            "Ethereum mainnet tick_rate should be eq to global tick_rate"
        );
        // Test Ethereum sepolia
        let ethereum_sepolia_providers = ethereum_net_provider.get("sepolia").unwrap();
        assert_eq!(
            ethereum_sepolia_providers.len(),
            1,
            "should have 1 provider for ethereum sepolia"
        );
        let ethereum_sepolia_rpc_urls = vec!["https://rpc-ethereum-6.com"];
        let ethereum_sepolia_rpc_providers = get_ethereum_nodes(ethereum_sepolia_providers);
        // Test first ethereum sepolia rpc url
        let b = ethereum_sepolia_rpc_providers
            .iter()
            .find(|x| x.endpoint.url == ethereum_sepolia_rpc_urls[0]);
        assert_eq!(
            b.is_some(),
            true,
            "First Ethereum sepolia rpc url should be set"
        );
        let e = b.unwrap().endpoint.clone();
        let e = e.reqwest;
        assert_eq!(
            e.config.retry, 25,
            "First Ethereum sepolia retry should be equal to 25"
        );
        assert_eq!(
            e.config.delay, 26,
            "First Ethereum sepolia delay should be equal to 26"
        );
        assert_eq!(
            e.config.rate, 27,
            "First Ethereum sepolia rate should be equal to 27"
        );
        let ethereum_sepolia_network_options = ethereum_network_options.get("sepolia").unwrap();
        assert_eq!(
            ethereum_sepolia_network_options.head_length, config.global.networks_options.head_length,
            "Ethereum sepolia head_length should be set to config.global.networks_options.head_length"
        );
        assert_eq!(
            ethereum_sepolia_network_options.tick_rate, config.global.networks_options.tick_rate,
            "Ethereum sepolia tick_rate should be set to config.global.networks_options.tick_rate"
        );
    }

    #[test]
    fn conf_simple() {
        // tests::setup();
        let args = vec![OsString::from("blockhead")];
        let config =
            Configuration::new(Some("./tests/light_config.yaml"), Some(args), false).unwrap();

        // Global values should match defaults
        assert_eq!(
            config.global.endpoints.rate, DEFAULT_ENDPOINT_REQUEST_RATE,
            "rate should be set to default value"
        );
        assert_eq!(
            config.global.endpoints.retry, DEFAULT_ENDPOINT_RETRY,
            "retry should be set to default value"
        );
        assert_eq!(
            config.global.endpoints.delay, DEFAULT_ENDPOINT_DELAY,
            "delay should be set to default value"
        );
        assert_eq!(
            config.global.server.port, DEFAULT_SERVER_PORT,
            "server port should be set to default value"
        );
        assert_eq!(
            config.global.metrics.port, DEFAULT_METRICS_PORT,
            "metrics port should be set to default value"
        );
        assert_eq!(
            config.global.networks_options.head_length, DEFAULT_HEAD_LENGTH,
            "head_length should be set to default value"
        );
        assert_eq!(
            config.global.networks_options.tick_rate, DEFAULT_TICK_RATE,
            "tick_rate should be set to default value"
        );
        assert_eq!(
            config.database.keep_history, DEFAULT_DATABASE_KEEP_HISTORY,
            "keep_history should be set to default value"
        );
        assert_eq!(
            config.database.path,
            PathBuf::from(DEFAULT_DATABASE_PATH),
            "db_path should be set to default value"
        );
        // Endpoint and network values should be set and match with default values
        let proto_providers = &config.proto_providers;
        assert_eq!(
            proto_providers.contains_key(&Protocol::Bitcoin),
            true,
            "Proto_provier should contain Bitcoin"
        );
        assert_eq!(
            proto_providers.keys().len(),
            1,
            "Proto_provier should contain only Bitcoin"
        );
        let bitcoin_provider = proto_providers.get(&Protocol::Bitcoin).unwrap();
        // should contain network mainnet
        assert_eq!(
            bitcoin_provider.contains_key("mainnet"),
            true,
            "bitcoin_provider should contain mainnet"
        );
        assert_eq!(
            bitcoin_provider.keys().len(),
            1,
            "bitcoin_provider should contain only mainnet"
        );
        // sould contain 2 providers
        assert_eq!(
            bitcoin_provider.get("mainnet").unwrap().len(),
            2,
            "bitcoin_provider mainnet should have 2 endpoints"
        );
        // Each provider should have default values
        let mainnet_providers = bitcoin_provider.get("mainnet").unwrap();
        let expected_urls = vec![
            "https://rpc-bitcoin-1.com".to_string(),
            "https://rpc-bitcoin-2.com".to_string(),
        ];
        for provider in mainnet_providers.iter() {
            match provider {
                Provider::BitcoinNode(r) => {
                    let r = r.clone();
                    assert_eq!(
                        expected_urls.contains(&r.endpoint.url),
                        true,
                        "Url should be one of the expected ones"
                    );
                    let client = r.endpoint.reqwest;

                    assert_eq!(
                        client.config.retry, DEFAULT_ENDPOINT_RETRY,
                        "Retry should be set to default"
                    );
                    assert_eq!(
                        client.config.delay, DEFAULT_ENDPOINT_DELAY,
                        "Delay should be set to default"
                    );
                    assert_eq!(
                        client.config.rate, DEFAULT_ENDPOINT_REQUEST_RATE,
                        "Rate should be set to default"
                    );
                }

                _ => assert!(false, "Provider should be BitcoinNode"),
            }
        }
    }
}
