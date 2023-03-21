use std::sync::Mutex;

use once_cell::sync::Lazy;
use warp::{Rejection, Reply};

use prometheus::Registry;

use super::metrics::{self, BLOCKCHAIN_HEAD_TIMESTAMP, BLOCKCHAIN_HEAD_TXS, BLOCKCHAIN_HEIGHT};

static REGISTRY: Lazy<Mutex<Registry>> = Lazy::new(|| Mutex::new(Registry::new()));

pub fn register_custom_metrics() {
    let r = REGISTRY.lock().unwrap();
    r.register(Box::new(metrics::HTTP_REQUEST_CODE_200.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::HTTP_REQUEST_CODE_400.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::HTTP_REQUEST_CODE_500.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEIGHT.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEAD_TIMESTAMP.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEAD_TXS.clone()))
        .expect("collector can be registered");
}
pub async fn metrics_handler() -> Result<impl Reply, Rejection> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&prometheus::gather(), &mut buffer) {
        eprintln!("could not encode prometheus metrics: {}", e);
    };
    let res = match String::from_utf8(buffer.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("prometheus metrics could not be from_utf8'd: {}", e);
            String::default()
        }
    };
    buffer.clear();
    Ok(res)
}
pub fn track_status_code(url: &str, status_code: u16, protocol: &str, network: &str) {
    let base_domain = url.split("/").next().unwrap();
    match status_code {
        500..=599 => metrics::HTTP_REQUEST_CODE_500
            .with_label_values(&[base_domain, protocol, network])
            .inc(),
        400..=499 => metrics::HTTP_REQUEST_CODE_400
            .with_label_values(&[base_domain, protocol, network])
            .inc(),
        200..=299 => metrics::HTTP_REQUEST_CODE_200
            .with_label_values(&[base_domain, protocol, network])
            .inc(),
        _ => (),
    };
}

pub fn set_blockchain_metrics(
    network: &str,
    protocol: &str,
    head_height: i64,
    head_time: i64,
    head_txs: i64,
) {
    BLOCKCHAIN_HEIGHT
        .with_label_values(&[protocol, network])
        .set(head_height);
    BLOCKCHAIN_HEAD_TIMESTAMP
        .with_label_values(&[protocol, network])
        .set(head_time);
    BLOCKCHAIN_HEAD_TXS
        .with_label_values(&[protocol, network])
        .set(head_txs);
}
