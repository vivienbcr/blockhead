use futures::future;
use std::process;
pub mod api;
pub mod collectors;
pub mod commons;
pub mod configuration;
pub mod db;
pub mod endpoints;
pub mod prom;
pub mod requests;
use crate::{
    api::{app, metrics},
    commons::blockchain::Blockchain,
    configuration::{NetworkName, ProtocolName},
    db::DATABASE,
    prom::registry::register_custom_metrics,
};
use db::Redb;
// use crate::configuration;
use actix_web::{get, middleware, post, web, App, HttpResponse, HttpServer, Responder};
#[macro_use]
extern crate log;

use env_logger::Env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env = Env::default().default_filter_or("blockhead=debug");

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
    let config = match configuration::Configuration::new() {
        Ok(c) => {
            info!("Configuration loaded successfully");
            c
        }
        Err(e) => {
            error!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };
    register_custom_metrics();

    config.protocols.iter().for_each(|protocol| {
        let proto_name = protocol.0.clone(); // Bitcoin, Ethereum, etc
        let map_networks = protocol.1.clone(); // mainnet, testnet, etc
        match &proto_name {
            configuration::ProtocolName::Bitcoin => {
                info!("Bitcoin endpoints detected... ");
                map_networks.iter().for_each(|map_network| {
                    let network = map_network.0.clone();
                    let endpoints = map_network.1.clone(); // At this point, ProtocolsOpts is only BitcoinOpts
                    match &endpoints {
                        configuration::ProtocolsOpts::Bitcoin(endpoints) => {
                            tokio::task::spawn(collectors::bitcoin(network, endpoints.clone()));
                        }
                        _ => {}
                    }
                })
            }
            configuration::ProtocolName::Ethereum => {
                info!("Ethereum collector not implemented yet");
            }
            _ => {}
        }
    });


    let metrics_port = config.global.metrics.port;
    let server_port = config.global.server.port;
    println!(
        "Started prometheus metrics server at http://localhost:{}/metrics",
        metrics_port
    );
    println!(
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
            .service(app::blockchain_handler)
    })
    .bind(("0.0.0.0", server_port))?
    .run();

    future::try_join(metrics_server, api).await?;

    Ok(())
}
