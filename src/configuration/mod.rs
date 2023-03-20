use std::collections::HashMap;

use config::{self, ConfigError, File};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::Value;

pub static CONFIGURATION: OnceCell<Configuration> = OnceCell::new();

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Configuration {
    pub global: Global,
    #[serde(deserialize_with = "deserialize_protocols")]
    pub protocols: HashMap<ProtocolName,HashMap<NetworkName,ProtoEndpoints>>,
}

fn deserialize_protocols<'de, D>(deserializer: D) -> Result<HashMap<ProtocolName, HashMap<NetworkName,ProtoEndpoints>>, D::Error>
where
    D: Deserializer<'de>,
{
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
            match proto_k {
                ProtocolName::Bitcoin => {
                    let endpoints = BitcoinEndpoints::deserialize(v).unwrap();
                    map.entry(proto_k.clone()).or_insert(HashMap::new()).insert(k, ProtoEndpoints::Bitcoin(endpoints));
                },
                ProtocolName::Ethereum => {
                    let endpoints = EthereumEndpoints::deserialize(v).unwrap();
                    map.entry(proto_k.clone()).or_insert(HashMap::new()).insert(k, ProtoEndpoints::Ethereum(endpoints));
                },
                ProtocolName::Tezos => {
                    let endpoints = TezosEndpoints::deserialize(v).unwrap();
                    map.entry(proto_k.clone()).or_insert(HashMap::new()).insert(k, ProtoEndpoints::Tezos(endpoints));
                },
            }
        }
    }
    Ok(map)
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
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Global {
    pub endpoints: EndpointOptions,
    pub metrics: Metrics,
    pub server: Server,
    pub head_length: u32,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Metrics {
    pub port: u16,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Server {
    pub port: u32,
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
        println!("s: {:?}", s);
        match s.as_str() {
            "bitcoin" => Ok(ProtocolName::Bitcoin),
            "ethereum" => Ok(ProtocolName::Ethereum),
            "tezos" => Ok(ProtocolName::Tezos),
            _ => Err(serde::de::Error::custom(format!("Unknown protocol: {}", s))),
        }
    }
}

#[derive( Serialize,Deserialize, Debug, Clone,Eq, Hash, PartialEq)]
pub enum ProtoEndpoints {

    Bitcoin(BitcoinEndpoints),
    Ethereum(EthereumEndpoints),
    Tezos(TezosEndpoints),
}


#[derive(Deserialize, Serialize, Debug, Clone,Eq, Hash, PartialEq)]
pub struct BitcoinEndpoints {
    pub rpc: Option<Vec<Endpoint>>,
    pub blockstream: Option<Endpoint>,
    pub blockcypher: Option<Endpoint>,
}
#[derive(Deserialize, Serialize, Debug, Clone,Eq, Hash, PartialEq)]
pub struct EthereumEndpoints {
    pub rpc: Option<Vec<Endpoint>>,
    pub infura: Option<Endpoint>,
}

#[derive(Deserialize, Serialize, Debug, Clone,Eq, Hash, PartialEq)]
pub struct TezosEndpoints {
    pub rpc: Option<Vec<Endpoint>>,
    pub tzstats: Option<Endpoint>,
    pub tzkt: Option<Endpoint>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Network {
    pub config: Option<EndpointOptions>,
    pub rpc: Option<Vec<Endpoint>>,
    pub blockstream: Option<Endpoint>,
    pub blockcypher: Option<Endpoint>,
}
#[derive(Deserialize, Serialize, Debug, Clone,Eq, Hash, PartialEq)]
pub struct Endpoint {
    pub url: String,
    pub options: Option<EndpointOptions>,
}
#[derive(Deserialize, Serialize, Debug, Clone,Eq, Hash, PartialEq)]
pub struct EndpointOptions {
    pub retry: Option<u32>,
    pub delay: Option<u32>,
    pub couldown: Option<u32>,
}

impl Configuration {
    pub fn new() -> Result<Self, ConfigError> {
        // TODO: config file should be overridable by env variables
        // TODO: config file should be overridable by cli args
        let builder = config::Config::builder()
            .add_source(File::with_name("config.yaml"))
            .set_default("global.server.port", 8080)?
            .set_default("global.metrics.port", 8081)?
            .set_default("global.head_length", 5)?
            .set_default("global.endpoints.retry", 3)?
            .set_default("global.endpoints.delay", 1)?
            .set_default("global.endpoints.couldown", 5)?
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
