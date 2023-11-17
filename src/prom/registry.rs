use std::sync::Mutex;

use once_cell::sync::Lazy;

use prometheus::Registry;

use crate::conf::{Network, Protocol};

use super::metrics::{
    self, BLOCKCHAIN_HEAD_TIMESTAMP, BLOCKCHAIN_HEAD_TXS, BLOCKCHAIN_HEIGHT,
    BLOCKCHAIN_HEIGHT_ENDPOINT,
};

static REGISTRY: Lazy<Mutex<Registry>> = Lazy::new(|| Mutex::new(Registry::new()));

pub fn register_custom_metrics() {
    let r = REGISTRY.lock().unwrap();
    r.register(Box::new(metrics::HTTP_REQUEST_CODE.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::HTTP_RESPONSE_TIME.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEIGHT.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEIGHT_ENDPOINT.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEAD_TIMESTAMP.clone()))
        .expect("collector can be registered");
    r.register(Box::new(metrics::BLOCKCHAIN_HEAD_TXS.clone()))
        .expect("collector can be registered");
}
pub fn track_status_code(
    url: &str,
    alias: &str,
    method: &str,
    status_code: u16,
    protocol: &Protocol,
    network: &Network,
) {
    trace!(
        "track status code {} {} {} {} {} {}",
        url,
        alias,
        method,
        status_code,
        protocol.to_string(),
        network.to_string()
    );

    let u = get_base_url(url);
    metrics::HTTP_REQUEST_CODE
        .with_label_values(&[
            &u,
            alias,
            &status_code.to_string(),
            method,
            &protocol.to_string(),
            &network.to_string(),
        ])
        .inc();
}

pub fn track_response_time(
    url: &str,
    alias: &str,
    method: &reqwest::Method,
    protocol: &Protocol,
    network: &Network,
    time: u128,
) {
    trace!(
        "track response time {} {} {} {} {} {}",
        url,
        alias,
        method,
        protocol,
        network,
        time as f64
    );
    let u = get_base_url(url);
    metrics::HTTP_RESPONSE_TIME
        .with_label_values(&[
            &u,
            alias,
            (method.as_ref()),
            &protocol.to_string(),
            &network.to_string(),
        ])
        .observe(time as f64);
}
pub fn set_blockchain_height_endpoint(
    url: &str,
    alias: &str,
    protocol: &Protocol,
    network: &Network,
    height: u64,
) {
    let u = get_base_url(url);
    BLOCKCHAIN_HEIGHT_ENDPOINT
        .with_label_values(&[&u, alias, &protocol.to_string(), &network.to_string()])
        .set(height as i64);
}

pub fn set_blockchain_metrics(
    protocol: &Protocol,
    network: &Network,
    head_height: i64,
    head_time: i64,
    head_txs: i64,
) {
    // If current height is higher than the new height, do not update
    let m_state = BLOCKCHAIN_HEIGHT
        .with_label_values(&[&protocol.to_string(), &network.to_string()])
        .get();
    if m_state >= head_height {
        return;
    }

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

pub fn set_endpoint_status_metric(
    url: &str,
    alias: &str,
    protocol: &Protocol,
    network: &Network,
    state: bool,
) {
    let u = get_base_url(url);
    let m_state = get_endpoint_status_metric(&u, alias, protocol, network);
    if m_state == state {
        return;
    }
    let state = if state { 1 } else { 0 };

    metrics::ENDPOINT_STATUS
        .with_label_values(&[&u, alias, &protocol.to_string(), &network.to_string()])
        .set(state);
}
fn get_base_url(url: &str) -> String {
    let base_url = url
        .split('/')
        .nth(2)
        .unwrap_or("unknown")
        .split('/')
        .next()
        .unwrap_or("unknown");
    base_url.to_string()
}
fn get_endpoint_status_metric(
    base_url: &str,
    alias: &str,
    protocol: &Protocol,
    network: &Network,
) -> bool {
    let state = metrics::ENDPOINT_STATUS
        .with_label_values(&[base_url, alias, &protocol.to_string(), &network.to_string()])
        .get();
    state == 1
}
#[cfg(test)]
mod test {
    use crate::prom::registry::get_base_url;

    #[test]
    fn test_prom_get_base_url() {
        assert_eq!(get_base_url("https://api.domain.tld"), "api.domain.tld");
        assert_eq!(
            get_base_url("https://api.domain.tld:1234"),
            "api.domain.tld:1234"
        );
        assert_eq!(
            get_base_url("https://api.domain.tld:1234/somethings"),
            "api.domain.tld:1234"
        );
        assert_eq!(
            get_base_url("https://foo.bar.api.domain.tld:1234/somethings/else"),
            "foo.bar.api.domain.tld:1234"
        );
    }
}
