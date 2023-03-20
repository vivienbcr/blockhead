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
    let config  = configuration::Configuration::new();
    let c=match config {
        Ok(c) => {
            println!("Configuration loaded");
            println!("{:?}", c);
            c
        },
        Err(e) => {
            println!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };
    register_custom_metrics();
    
    let metrics_route = warp::path!("metrics").and_then(metrics_handler);
    // let bitcoin_mainnet_url = c.

    // tokio::task::spawn(collectors::bitcoin::collect(
    //     bitcoin_mainnet_url,
    //     "mainnet".to_string(),
    // ));

    println!("Started prometheus metrics server at http://localhost:{}/metrics" , c.global.metrics.port);
    let prom_port = c.global.metrics.port;
    warp::serve(metrics_route).run(([0, 0, 0, 0], prom_port)).await;
}
