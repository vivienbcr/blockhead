use warp::Filter;
pub mod prom;
pub mod requests;
pub mod commons;
pub mod configuration;
pub mod collectors;
pub mod endpoints;
use crate::prom::registry::{metrics_handler, register_custom_metrics};
// use crate::configuration;


#[tokio::main]
async fn main() {
    let config  =  match configuration::Configuration::new() {
        Ok(c) => {
            println!("Configuration successfully loaded..");
            c
        },
        Err(e) => {
            println!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };
    println!("\n\nConfiguration: {:?}\n\n", config);
    register_custom_metrics();
    
    let metrics_route = warp::path!("metrics").and_then(metrics_handler);
    
    
    // for each protocol in config, for each network in protocol, spawn a collector
    config.protocols.iter().for_each(|protocol| {
        let proto_name = protocol.0.clone(); // Bitcoin, Ethereum, etc
        let map_networks = protocol.1.clone(); // mainnet, testnet, etc
        match &proto_name {
            configuration::ProtocolName::Bitcoin => {
                println!("Spawning Bitcoin collector");
                map_networks.iter().for_each(|map_network| {
                    let network = map_network.0.clone();
                    let endpoints = map_network.1.clone(); // At this point, protoEndpoints is only BitcoinEndpoints
                    match &endpoints {
                        configuration::ProtoEndpoints::Bitcoin(endpoints) => {
                            println!("Spawning collector for protocol: {:?}, network: {:?}, endpoints: {:?}", proto_name, network, endpoints);
                            tokio::task::spawn(collectors::bitcoin(network, endpoints.clone()));
                        },
                        _ => {}
                    }
                    
                    // println!("Spawning collector for protocol: {:?}, network: {:?}", protocol, network);
                    // tokio::task::spawn(collectors::bitcoin(network, endpoints));
                })
            },
            configuration::ProtocolName::Ethereum => {
                println!("Ethereum collector not implemented yet");
            },
            _ => {}

        }

        // protocol.1.iter().for_each(|map_network| {
        //     let protocol = protocol.0.clone();
        //     let network = map_network.0.clone();
        //     let endpoints = map_network.1.clone();
        //     println!("Spawning collector for protocol: {:?}, network: {:?}", protocol, network);
        //     tokio::task::spawn(collectors::collector(protocol, network, endpoints));
        // })
    });
    
    // let bitcoin_mainnet_url = c.
 

    // tokio::task::spawn(collectors::bitcoin::collect(
    //     bitcoin_mainnet_url,
    //     "mainnet".to_string(),
    // ));

    println!("Started prometheus metrics server at http://localhost:{}/metrics" , config.global.metrics.port);
    let prom_port = config.global.metrics.port;
    warp::serve(metrics_route).run(([0, 0, 0, 0], prom_port)).await;
}
