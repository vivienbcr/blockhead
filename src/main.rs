use futures::future;
pub mod api;
pub mod collectors;
pub mod commons;
pub mod conf2;
pub mod configuration;
pub mod db;
pub mod endpoints;
pub mod prom;
pub mod requests;
#[cfg(test)]
pub mod tests;

use crate::{
    api::{app, metrics},
    prom::registry::register_custom_metrics,
};
use db::Redb;
// use crate::configuration;
use actix_web::{middleware, App, HttpServer};
#[macro_use]
extern crate log;

use env_logger::Env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env = Env::default().default_filter_or("blockhead=trace");

    match Redb::init() {
        Ok(_) => {
            info!("Redb db is initialized");
        }
        Err(e) => {
            error!("Redb db is not created {:?}", e);
            std::process::exit(1);
        }
    }

    env_logger::init_from_env(env);
    register_custom_metrics();

    let config = conf2::Configuration::new("config.yaml").unwrap();
    let protocols_networks = config.proto_providers.clone();
    protocols_networks.iter().for_each(|n| {
        let protocol = n.0.clone();
        let networks_map = n.1.clone();
        networks_map.iter().for_each(|n| {
            let network = n.0.clone();
            let network_options = config.get_network_options(&protocol, &network).unwrap();
            let providers = n.1.clone();

            tokio::task::spawn(collectors::runner(
                protocol.clone(),
                network.clone(),
                providers.clone(),
                network_options.clone(),
            ));
        })
    });

    // let config = match configuration::Configuration::new() {
    //     Ok(c) => {
    //         info!("Configuration loaded successfully");
    //         c
    //     }
    //     Err(e) => {
    //         error!("Error loading configuration: {}", e);
    //         std::process::exit(1);
    //     }
    // };

    // config.protocols.iter().for_each(|protocol| {
    //     let proto_name = protocol.0.clone(); // Bitcoin, Ethereum, etc
    //     let map_networks = protocol.1.clone(); // mainnet, testnet, etc
    //     match &proto_name {
    //         configuration::ProtocolName::Bitcoin => {
    //             info!("Bitcoin endpoints detected... ");
    //             map_networks.iter().for_each(|map_network| {
    //                 let network = map_network.0.clone();
    //                 let endpoints = map_network.1.clone(); // At this point, ProtocolsOpts is only BitcoinOpts
    //                 match &endpoints {
    //                     configuration::ProtocolsOpts::Bitcoin(endpoints) => {
    //                         tokio::task::spawn(bitcoin::bitcoin(network, endpoints.clone()));
    //                     }
    //                     _ => {}
    //                 }
    //             })
    //         }
    //         configuration::ProtocolName::Ethereum => {
    //             info!("Ethereum endpoints detected... ");
    //             map_networks.iter().for_each(|map_network| {
    //                 let network = map_network.0.clone();
    //                 let endpoints = map_network.1.clone(); // At this point, ProtocolsOpts is only BitcoinOpts
    //                 match &endpoints {
    //                     configuration::ProtocolsOpts::Ethereum(endpoints) => {
    //                         tokio::task::spawn(ethereum::ethereum(network, endpoints.clone()));
    //                     }
    //                     _ => {}
    //                 }
    //             })
    //         }
    //         _ => {}
    //     }
    // });
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        std::process::exit(1);
    }));
    let metrics_port = config.global.metrics.port;
    let server_port = config.global.server.port;
    info!(
        "Started prometheus metrics server at http://localhost:{}/metrics",
        metrics_port
    );
    info!(
        "Started blockhead server at http://localhost:{}/",
        server_port
    );
    let metrics_server = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Compress::default())
            .service(metrics::prometheus_handler)
    })
    .bind(("0.0.0.0", metrics_port))?
    .run();
    let api = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Compress::default())
            .service(app::ping_handler)
    })
    .bind(("0.0.0.0", server_port))?
    .run();

    future::try_join(metrics_server, api).await?;

    Ok(())
}
