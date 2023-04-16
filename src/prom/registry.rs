use std::sync::Mutex;

use once_cell::sync::Lazy;

use prometheus::Registry;

use crate::{
    conf2::{Network2, Protocol2},
    configuration::{NetworkName, ProtocolName},
};

use super::metrics::{self, BLOCKCHAIN_HEAD_TIMESTAMP, BLOCKCHAIN_HEAD_TXS, BLOCKCHAIN_HEIGHT};

static REGISTRY: Lazy<Mutex<Registry>> = Lazy::new(|| Mutex::new(Registry::new()));
//TODO: Monitor response time for each endpoint
pub fn register_custom_metrics() {
    let r = REGISTRY.lock().unwrap();
    r.register(Box::new(metrics::HTTP_REQUEST_CODE.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::ENDPOINT_RESPONSE_TIME.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEIGHT.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEAD_TIMESTAMP.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEAD_TXS.clone()))
        .expect("collector can be registered");
}
pub fn track_status_code(url: &str, method: &str, status_code: u16, protocol: &str, network: &str) {
    // retain only https://domain.tld
    let base_domain = url
        .split('/')
        .nth(2)
        .unwrap_or("unknown")
        .split(':')
        .nth(0)
        .unwrap_or("unknown");

    metrics::HTTP_REQUEST_CODE
        .with_label_values(&[
            base_domain,
            &status_code.to_string(),
            method,
            protocol,
            network,
        ])
        .inc();
}

pub fn track_response_time(url: &str, method: &str, protocol: &str, network: &str, time: f64) {
    // retain only https://domain.tld
    let base_domain = url
        .split('/')
        .nth(2)
        .unwrap_or("unknown")
        .split(':')
        .nth(0)
        .unwrap_or("unknown");
    metrics::ENDPOINT_RESPONSE_TIME
        .with_label_values(&[base_domain, method, protocol, network])
        .observe(time);
}

pub fn set_blockchain_metrics(
    protocol: Protocol2,
    network: Network2,
    head_height: i64,
    head_time: i64,
    head_txs: i64,
) {
    BLOCKCHAIN_HEIGHT
        .with_label_values(&[&protocol.to_string(), &network.to_string()])
        .set(head_height);
    BLOCKCHAIN_HEAD_TIMESTAMP
        .with_label_values(&[&protocol.to_string(), &network.to_string()])
        .set(head_time);
    BLOCKCHAIN_HEAD_TXS
        .with_label_values(&[&protocol.to_string(), &network.to_string()])
        .set(head_txs);
}
