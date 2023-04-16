use crate::{
    configuration::{Metrics, Server},
    endpoints::{
        bitcoin_node::BitcoinNode, blockcypher::Blockcypher, blockstream::Blockstream,
        ethereum_node::EthereumNode, ProviderActions,
    },
};
use config::{self, ConfigError, File};

use once_cell::sync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub static CONFIGURATION: OnceCell<Configuration> = OnceCell::new();
// pub static CONFIGURATION_GLOB_ENDPOINT_OPTION: OnceCell<EndpointOptions> = OnceCell::new();
// pub static CONFIGURATION_GLOB_NETWORK_OPTION: OnceCell<NetworkAppOptions> = OnceCell::new();
type NetworkProvider = HashMap<Network2, Vec<Provider>>;
type NetworkOptions = HashMap<Network2, NetworkAppOptions>;
type ProtocolsNetworksOpts = HashMap<Protocol2, NetworkOptions>;
type ProtocolsNetworksProviders = HashMap<Protocol2, NetworkProvider>;
struct ProtoOptsProvider {
    pub proto_opts: ProtocolsNetworksOpts,
    pub proto_providers: ProtocolsNetworksProviders,
}

/**
 * Configuration is the main struct used to store all configuration
 */
#[derive(Serialize, Debug, Clone)]
pub struct Configuration {
    pub global: Global,
    pub database: Database,
    pub proto_opts: ProtocolsNetworksOpts,
    pub proto_providers: ProtocolsNetworksProviders,
}
impl Configuration {
    pub fn get_network_options(
        &self,
        protocol: &Protocol2,
        network: &Network2,
    ) -> Option<&NetworkAppOptions> {
        self.proto_opts.get(protocol)?.get(network)
    }
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
            let protocol = Protocol2::from(proto.clone());
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
                let network = Network2::from(net.clone());
                let network = match network {
                    Some(n) => n,
                    _ => {
                        panic!("Unkonwn protocol: {} found in configuration file", proto)
                    }
                };
                // Deserialize providers and network options
                let o: Value = serde_json::from_str(&opts.to_string()).unwrap();
                o.as_object().unwrap().iter().for_each(|(provider, opt)| {
                    let mut providers = Vec::new();
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
                            providers.push(provider);
                        }
                    }
                    net_providers.insert(network.clone(), providers);
                });
            });
            // After deserialization, if user didn't specify network options we use global options
            if net_opts.is_empty() {
                let mut net_opt = global.networks_options.clone();
                net_opts.insert(Network2::Mainnet, net_opt);
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
// deserialize_global_endpoint_options
// Is used to init global endpoint options, this Global will be consider as default values for all endpoints
// it should be deserialized first and set global to be reused by all endpoints deserialization and initialization
// fn deserialize_global_endpoint_options<'de, D>(deserializer: D) -> Result<EndpointOptions, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let endpoint_opt = EndpointOptions::deserialize(deserializer).unwrap();
//     debug!("deserialize_global_endpoint_options: {:?}", endpoint_opt);
//     CONFIGURATION_GLOB_ENDPOINT_OPTION
//         .set(endpoint_opt.clone())
//         .unwrap();
//     debug!(
//         "set CONFIGURATION_GLOB_ENDPOINT_OPTION: {:?}",
//         CONFIGURATION_GLOB_ENDPOINT_OPTION.get().unwrap()
//     );
//     Ok(endpoint_opt)
// }
// deserialize_global_network_options
// Is used to init global network options, this Global will be consider as default values for all endpoints
// it should be deserialized first and set global to be reused by all endpoints deserialization and initialization
// fn deserialize_global_network_options<'de, D>(
//     deserializer: D,
// ) -> Result<NetworkAppOptions, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let network_opt = NetworkAppOptions::deserialize(deserializer).unwrap();
//     debug!("deserialize_global_network_options: {:?}", network_opt);
//     CONFIGURATION_GLOB_NETWORK_OPTION
//         .set(network_opt.clone())
//         .unwrap();
//     debug!(
//         "set CONFIGURATION_GLOB_NETWORK_OPTION: {:?}",
//         CONFIGURATION_GLOB_NETWORK_OPTION.get().unwrap()
//     );
//     Ok(network_opt)
// }

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Database {
    pub keep_history: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum Provider {
    Blockstream(Blockstream),
    Blockcypher(Blockcypher),
    BitcoinNode(BitcoinNode),
    EthereumNode(EthereumNode),
    None,
}
impl Provider {
    pub fn from_str(provider: &str, endpoint_opt: EndpointOptions, network: &Network2) -> Provider {
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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
}

#[derive(Deserialize, Serialize, Debug, Clone, Eq, Hash, PartialEq, Copy)]
pub enum Protocol2 {
    #[serde(rename = "bitcoin")]
    Bitcoin,
    #[serde(rename = "ethereum")]
    Ethereum,
    #[serde(rename = "tezos")]
    Tezos,
    #[serde(rename = "None")]
    None,
}

impl Protocol2 {
    fn from(s: String) -> Option<Self> {
        match s.as_str() {
            "bitcoin" => Some(Protocol2::Bitcoin),
            "ethereum" => Some(Protocol2::Ethereum),
            "tezos" => Some(Protocol2::Tezos),
            _ => None,
        }
    }
    pub fn to_string(&self) -> String {
        match self {
            Protocol2::Bitcoin => "bitcoin".to_string(),
            Protocol2::Ethereum => "ethereum".to_string(),
            Protocol2::Tezos => "tezos".to_string(),
            Protocol2::None => "None".to_string(),
        }
    }
}
#[derive(Deserialize, Serialize, Debug, Clone, Eq, Hash, PartialEq, Copy)]
pub enum Network2 {
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
impl Network2 {
    fn from(s: String) -> Option<Self> {
        match s.as_str() {
            "mainnet" => Some(Network2::Mainnet),
            "testnet" => Some(Network2::Testnet),
            "goerli" => Some(Network2::Goerli),
            "sepolia" => Some(Network2::Sepolia),
            "ghostnet" => Some(Network2::Ghostnet),
            _ => None,
        }
    }
    pub fn to_string(&self) -> String {
        match self {
            Network2::Mainnet => "mainnet".to_string(),
            Network2::Testnet => "testnet".to_string(),
            Network2::Goerli => "goerli".to_string(),
            Network2::Sepolia => "sepolia".to_string(),
            Network2::Ghostnet => "ghostnet".to_string(),
        }
    }
}

const DEFAULT_SERVER_PORT: u32 = 8080;
const DEFAULT_METRICS_PORT: u16 = 8081;
const DEFAULT_HEAD_LENGTH: u32 = 5;
const DEFAULT_TICK_RATE: u32 = 5;
const DEFAULT_ENDPOINT_RETRY: u32 = 3;
const DEFAULT_ENDPOINT_DELAY: u32 = 1;
const DEFAULT_ENDPOINT_REQUEST_RATE: u32 = 5;
const DEFAULT_DATABASE_KEEP_HISTORY: u32 = 1000;

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
    use crate::tests;

    #[tokio::test]
    async fn test_config() {
        tests::setup();
        let config = super::Configuration::new("./tests/sample_config.yaml").unwrap();
        println!("{:?}", config);
    }
}
