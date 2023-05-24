use std::process;

use conf::get_configuration;
use futures::future;
pub mod api;
pub mod collectors;
pub mod commons;
pub mod conf;
pub mod db;
pub mod endpoints;
pub mod prom;
pub mod requests;
pub mod utils;
use actix_cors::Cors;
#[cfg(test)]
pub mod tests;

use crate::{
    api::{app, metrics},
    prom::registry::register_custom_metrics,
};
use actix_web::{middleware, App, HttpServer};
use db::Redb;
#[macro_use]
extern crate log;

fn run_tasks() -> Vec<tokio::task::JoinHandle<()>> {
    let mut tasks = vec![];
    let config = get_configuration().unwrap();
    let protocols_networks = config.proto_providers.clone();
    protocols_networks.iter().for_each(|n| {
        let protocol = n.0.clone();
        let networks_map = n.1.clone();
        networks_map.iter().for_each(|n| {
            let network = n.0.clone();
            let network_options = config
                .get_network_options(&protocol, &network)
                .cloned()
                .unwrap();
            let providers = n.1.clone();
            let r = tokio::task::spawn({
                collectors::runner(
                    protocol.clone(),
                    network.clone(),
                    providers.clone(),
                    network_options,
                )
            });
            tasks.push(r);
        })
    });
    tasks
}

async fn start_tasks(mut rx: mpsc::Receiver<bool>) -> Result<(), io::Error> {
    let mut tasks = run_tasks();
    while let Some(_) = rx.recv().await {
        info!("Configuration changed, restarting {} tasks", tasks.len());
        for t in tasks.iter() {
            t.abort();
        }
        tasks = run_tasks();
    }
    Ok(())
}

use tokio::{io, signal, sync::mpsc};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    conf::init_logger(None);
    register_custom_metrics();
    let config = conf::Configuration::new(None, None, true).unwrap();
    let (tx, rx) = mpsc::channel(1);
    match Redb::init(&config.database) {
        Ok(_) => {
            info!("Redb db is initialized");
        }
        Err(e) => {
            error!("Redb db is not created {:?}", e);
            std::process::exit(1);
        }
    }
    let conf_watcher = conf::watch_configuration_change(tx);
    let scrapp = start_tasks(rx);

    // Should panic if one of the task panic
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
        // // FIXME: cors from config
        let cors = Cors::default()
            .allow_any_origin()
            .send_wildcard()
            .allowed_methods(vec!["GET"])
            .allow_any_header()
            .max_age(3600);
        App::new()
            .wrap(cors)
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .service(metrics::prometheus_handler)
    })
    .bind(("0.0.0.0", metrics_port))?
    .run();
    let api = HttpServer::new(move || {
        //FIXME: cors from config
        let cors = Cors::default()
            .allow_any_origin()
            .send_wildcard()
            .allowed_methods(vec!["GET"])
            .allow_any_header()
            .max_age(3600);
        App::new()
            .wrap(cors)
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .service(app::protocols_handler)
            .service(app::protocol_handler)
            .service(app::protocol_net_handler)
    })
    .bind(("0.0.0.0", server_port))?
    .run();

    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Received Ctrl+C, exiting");
                process::exit(0x0100);
            }
            Err(err) => {
                error!("Unable to listen for shutdown signal: {}", err);
                process::exit(0x0100);
            }
        }
    });

    future::try_join4(metrics_server, api, scrapp, conf_watcher).await?;

    Ok(())
}
