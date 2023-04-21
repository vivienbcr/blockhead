use futures::future;
pub mod api;
pub mod collectors;
pub mod commons;
pub mod conf;
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
use actix_web::{middleware, App, HttpServer};
use db::Redb;
#[macro_use]
extern crate log;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    conf::init_logger(None);
    register_custom_metrics();
    let config = conf::Configuration::new(None, None, true).unwrap();
    match Redb::init(&config.database) {
        Ok(_) => {
            info!("Redb db is initialized");
        }
        Err(e) => {
            error!("Redb db is not created {:?}", e);
            std::process::exit(1);
        }
    }

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
            .service(app::blockchain_handler)
    })
    .bind(("0.0.0.0", server_port))?
    .run();

    future::try_join(metrics_server, api).await?;

    Ok(())
}
