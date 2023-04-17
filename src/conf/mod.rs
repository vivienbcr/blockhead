use crate::{
    endpoints::{
        bitcoin_node::BitcoinNode, blockcypher::Blockcypher, blockstream::Blockstream,
        ethereum_node::EthereumNode, ProviderActions,
    },
    requests::client::ReqwestClient,
};
use config::{self, ConfigError, File};

use once_cell::sync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

pub static CONFIGURATION: OnceCell<Configuration> = OnceCell::new();
// pub static CONFIGURATION_GLOB_ENDPOINT_OPTION: OnceCell<EndpointOptions> = OnceCell::new();
// pub static CONFIGURATION_GLOB_NETWORK_OPTION: OnceCell<NetworkAppOptions> = OnceCell::new();
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
pub fn get_enabled_protocol_network() -> HashMap<Protocol, Vec<Network>> {
    let config = CONFIGURATION.get().unwrap();
    let mut res = HashMap::new();
    for (proto, networks) in &config.proto_opts {
        let mut net_names = Vec::new();
        for (net, _) in networks {
            net_names.push(net.clone().into());
        }
        res.insert(proto.clone().into(), net_names);
    }
    res
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
        // let protocols =
        //     deserialize_protocols(v.as_object().unwrap().get("protocols").unwrap()).unwrap();
        // let mut enabled_proto_net = HashMap::new();
        // protocols.iter().for_each(|(proto, net)| {
        //     enabled_proto_net.insert(proto.clone(), net.keys().cloned().collect());
        // });

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
    // proto_opts store network options for each protocol
    let mut proto_opts: ProtocolsNetworksOpts = HashMap::new();
    // proto_providers store providers for each protocol
    let mut proto_providers: ProtocolsNetworksProviders = HashMap::new();
    // Deserialize protocols
    v.as_object()
        .unwrap()
        .iter()
        .for_each(|(proto, proto_config)| {
            debug!("Deserialize protocol {}", proto);
            let mut net_opts: NetworkOptions = HashMap::new();
            let mut net_providers: NetworkProvider = HashMap::new();
            let protocol = Protocol::from(proto.clone());
            let protocol = match protocol {
                Some(p) => p,
                _ => {
                    panic!("Unkonwn protocol: {} found in configuration file", proto)
                }
            };
            // Deserialise Network
            let s: Value = serde_json::from_str(&proto_config.to_string()).unwrap();
            s.as_object().unwrap().iter().for_each(|(net, opts)| {
                debug!("Deserialize network {}", net);
                let network = Network::from(net.clone());
                let network = match network {
                    Some(n) => n,
                    _ => {
                        panic!("Unkonwn protocol: {} found in configuration file", proto)
                    }
                };
                // Deserialize providers and network options
                let mut providers = Vec::new();
                let o: Value = serde_json::from_str(&opts.to_string()).unwrap();
                o.as_object().unwrap().iter().for_each(|(provider, opt)| {
                    let endpoints_options: Value = serde_json::from_str(&opt.to_string()).unwrap();
                    match provider.as_str() {
                        // At deserialization, if user specify network options we merge them with global options
                        "network_options" => {
                            let mut net_opt = global.networks_options.clone();
                            net_opt
                                .merge(NetworkAppOptions::deserialize(opt).unwrap())
                                .unwrap();
                            net_opts.insert(network.clone(), net_opt);
                        }
                        // Rpc is vec of provider declaration,
                        "rpc" => {
                            debug!("Found rpc {}", provider);
                            endpoints_options
                                .as_array()
                                .unwrap_or(&Vec::new())
                                .iter()
                                .for_each(|endpoint| {
                                    // merge endpoint options with global endpoint options
                                    println!("endpoint: {:?}", endpoint);
                                    let provider_config_f =
                                        ProviderConfigF::deserialize(endpoint).unwrap();

                                    let endpoint_opts =
                                        EndpointOptions::from_provider_config_f(provider_config_f);
                                    let mut default_endpoint_opt = global.endpoints.clone();

                                    default_endpoint_opt.merge(endpoint_opts).unwrap();
                                    debug!("endpoint_opt: {:?}", default_endpoint_opt);
                                    let rpc_provider = Provider::from_str(
                                        &format!("{}_node", protocol.to_string()),
                                        default_endpoint_opt,
                                        &network,
                                    );
                                    providers.push(rpc_provider);
                                })
                        }
                        // Str is a provider declaration
                        str => {
                            debug!("Found provider {}", str);
                            let provider_config_f = ProviderConfigF::deserialize(opt).unwrap();
                            let endpoint_opts =
                                EndpointOptions::from_provider_config_f(provider_config_f);
                            let mut default_endpoint_opt = global.endpoints.clone();

                            default_endpoint_opt.merge(endpoint_opts).unwrap();
                            debug!("endpoint_opt: {:?}", default_endpoint_opt);

                            let provider = Provider::from_str(str, default_endpoint_opt, &network);
                            debug!("provider: {:?}", provider);
                            providers.push(provider);
                        }
                    }
                });
                debug!(
                    "Protocol: {} Network: {} Providers: {}",
                    protocol.to_string(),
                    network.to_string(),
                    providers.len()
                );
                net_providers.insert(network.clone(), providers);
            });
            // After deserialization, if user didn't specify network options we use global options
            if net_opts.is_empty() {
                let net_opt = global.networks_options.clone();
                net_opts.insert(Network::Mainnet, net_opt);
            }

            proto_opts.insert(protocol.clone(), net_opts);
            proto_providers.insert(protocol.clone(), net_providers);
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
    // #[serde(deserialize_with = "deserialize_global_endpoint_options")]
    pub endpoints: EndpointOptions,
    // #[serde(deserialize_with = "deserialize_global_network_options")]
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
    pub keep_history: u32,
}

#[derive(Debug, Clone)]
pub enum Provider {
    Blockstream(Blockstream),
    Blockcypher(Blockcypher),
    BitcoinNode(BitcoinNode),
    EthereumNode(EthereumNode),
    None,
}
impl Provider {
    pub fn from_str(provider: &str, endpoint_opt: EndpointOptions, network: &Network) -> Provider {
        let n = network.to_owned();
        match provider {
            "blockstream" => Provider::Blockstream(Blockstream::new(endpoint_opt, n)),
            "blockcypher" => Provider::Blockcypher(Blockcypher::new(endpoint_opt, n)),
            "bitcoin_node" => Provider::BitcoinNode(BitcoinNode::new(endpoint_opt, n)),
            "ethereum_node" => Provider::EthereumNode(EthereumNode::new(endpoint_opt, n)),
            _ => Provider::None,
        }
    }

    pub fn as_mut_provider_actions(&mut self) -> Option<&mut dyn ProviderActions> {
        match self {
            Provider::Blockstream(provider) => Some(provider),
            Provider::Blockcypher(provider) => Some(provider),
            Provider::BitcoinNode(provider) => Some(provider),
            Provider::EthereumNode(provider) => Some(provider),
            // Provider::None => None,
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EthereumProviders {
    Rpc(Vec<EthereumNode>),
}
/**
 * Network options is used to define network specific options
 * With this you can fine tune the network scraping params on your needs
 */
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct NetworkAppOptions {
    pub head_length: Option<u32>,
    pub tick_rate: Option<u32>,
}
impl NetworkAppOptions {
    pub fn merge(&mut self, other: NetworkAppOptions) -> Result<(), ConfigError> {
        if let Some(head_length) = other.head_length {
            self.head_length = Some(head_length);
        }
        if let Some(tick_rate) = other.tick_rate {
            self.tick_rate = Some(tick_rate);
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
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProviderConfigF {
    pub url: Option<String>,
    pub options: Option<ProviderOptsConfigF>,
}

/**
 * Endpoint options is used to define reqwest client options
 */
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EndpointOptions {
    pub url: Option<String>,
    pub retry: Option<u32>,
    pub delay: Option<u32>,
    pub rate: Option<u32>,
}

impl EndpointOptions {
    //TODO! impl default
    pub fn from_provider_config_f(provider: ProviderConfigF) -> EndpointOptions {
        let mut endpoint_opt = EndpointOptions {
            url: None,
            retry: None,
            delay: None,
            rate: None,
        }; //TODO use default
        if let Some(url) = provider.url {
            endpoint_opt.url = Some(url);
        }
        if let Some(options) = provider.options {
            if let Some(retry) = options.retry {
                endpoint_opt.retry = Some(retry);
            }
            if let Some(delay) = options.delay {
                endpoint_opt.delay = Some(delay);
            }
            if let Some(rate) = options.rate {
                endpoint_opt.rate = Some(rate);
            }
        }
        endpoint_opt
    }
    pub fn merge(&mut self, other: EndpointOptions) -> Result<(), ConfigError> {
        if let Some(url) = other.url {
            self.url = Some(url);
        }
        if let Some(retry) = other.retry {
            self.retry = Some(retry);
        }
        if let Some(delay) = other.delay {
            self.delay = Some(delay);
        }
        if let Some(rate) = other.rate {
            self.rate = Some(rate);
        }
        Ok(())
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

#[derive(Deserialize, Serialize, Debug, Clone, Eq, Hash, PartialEq, Copy)]
pub enum Protocol {
    #[serde(rename = "bitcoin")]
    Bitcoin,
    #[serde(rename = "ethereum")]
    Ethereum,
    #[serde(rename = "tezos")]
    Tezos,
    #[serde(rename = "None")]
    None,
}

impl Protocol {
    fn from(s: String) -> Option<Self> {
        match s.as_str() {
            "bitcoin" => Some(Protocol::Bitcoin),
            "ethereum" => Some(Protocol::Ethereum),
            "tezos" => Some(Protocol::Tezos),
            _ => None,
        }
    }
    pub fn to_string(&self) -> String {
        match self {
            Protocol::Bitcoin => "bitcoin".to_string(),
            Protocol::Ethereum => "ethereum".to_string(),
            Protocol::Tezos => "tezos".to_string(),
            Protocol::None => "None".to_string(),
        }
    }
}
#[derive(Deserialize, Serialize, Debug, Clone, Eq, Hash, PartialEq, Copy)]
pub enum Network {
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
}
impl Network {
    fn from(s: String) -> Option<Self> {
        match s.as_str() {
            "mainnet" => Some(Network::Mainnet),
            "testnet" => Some(Network::Testnet),
            "goerli" => Some(Network::Goerli),
            "sepolia" => Some(Network::Sepolia),
            "ghostnet" => Some(Network::Ghostnet),
            _ => None,
        }
    }
    pub fn to_string(&self) -> String {
        match self {
            Network::Mainnet => "mainnet".to_string(),
            Network::Testnet => "testnet".to_string(),
            Network::Goerli => "goerli".to_string(),
            Network::Sepolia => "sepolia".to_string(),
            Network::Ghostnet => "ghostnet".to_string(),
        }
    }
}
#[derive(Serialize, Debug, Clone)]
pub struct Endpoint {
    pub url: String,
    // pub options: Option<EndpointOptions>,
    #[serde(skip)]
    pub reqwest: Option<ReqwestClient>,
    #[serde(skip)]
    pub network: Network,
    #[serde(skip)]
    pub last_request: u64,
}
impl Endpoint {
    pub fn test_new(url: &str, net: Network) -> Self {
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
pub const DEFAULT_SERVER_PORT: u32 = 8080;
pub const DEFAULT_METRICS_PORT: u16 = 8081;
pub const DEFAULT_HEAD_LENGTH: u32 = 5;
pub const DEFAULT_TICK_RATE: u32 = 5;
pub const DEFAULT_ENDPOINT_RETRY: u32 = 3;
pub const DEFAULT_ENDPOINT_DELAY: u32 = 1;
pub const DEFAULT_ENDPOINT_REQUEST_RATE: u32 = 5;
pub const DEFAULT_DATABASE_KEEP_HISTORY: u32 = 1000;

impl Configuration {
    pub fn new(file: &str) -> Result<Self, ConfigError> {
        let builder = config::Config::builder()
            .set_default("global.server.port", DEFAULT_SERVER_PORT)?
            .set_default("global.metrics.port", DEFAULT_METRICS_PORT)?
            .set_default("global.networks_options.head_length", DEFAULT_HEAD_LENGTH)?
            .set_default("global.networks_options.tick_rate", DEFAULT_TICK_RATE)?
            .set_default("database.keep_history", DEFAULT_DATABASE_KEEP_HISTORY)?
            .set_default("global.endpoints.retry", DEFAULT_ENDPOINT_RETRY)?
            .set_default("global.endpoints.delay", DEFAULT_ENDPOINT_DELAY)?
            .set_default("global.endpoints.rate", DEFAULT_ENDPOINT_REQUEST_RATE)?
            .add_source(File::with_name(file))
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
}

#[cfg(test)]
mod test {
    use crate::{conf::Configuration, tests};

    #[tokio::test]
    async fn test_config() {
        tests::setup();
        let config = Configuration::new("./tests/sample_config.yaml").unwrap();
        println!("{:?}", config);
    }
}
