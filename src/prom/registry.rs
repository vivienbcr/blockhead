use std::sync::Mutex;

use once_cell::sync::Lazy;

use prometheus::Registry;

use crate::conf::{Network, Protocol};

use super::metrics::{
    self, BLOCKCHAIN_HEAD_TIMESTAMP, BLOCKCHAIN_HEAD_TXS, BLOCKCHAIN_HEIGHT,
    BLOCKCHAIN_HEIGHT_ENDPOINT,
};

static REGISTRY: Lazy<Mutex<Registry>> = Lazy::new(|| Mutex::new(Registry::new()));
//TODO: Monitor response time for each endpoint
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
    method: &str,
    status_code: u16,
    protocol: &Protocol,
    network: &Network,
) {
    trace!(
        "track status code {} {} {} {} {}",
        url,
        method,
        status_code,
        protocol.to_string(),
        network.to_string()
    );
    // retain only https://domain.tld
    let base_domain = get_base_url(url);
    metrics::HTTP_REQUEST_CODE
        .with_label_values(&[
            &base_domain,
            &status_code.to_string(),
            method,
            &protocol.to_string(),
            &network.to_string(),
        ])
        .inc();
}

pub fn track_response_time(
    url: &str,
    method: &reqwest::Method,
    protocol: &Protocol,
    network: &Network,
    time: u128,
) {
    // retain only https://domain.tld
    let base_domain = get_base_url(url);
    trace!(
        "track response time{} {} {} {} {}",
        base_domain,
        method,
        protocol,
        network,
        time as f64
    );
    metrics::HTTP_RESPONSE_TIME
        .with_label_values(&[
            &base_domain,
            (method.as_ref()),
            &protocol.to_string(),
            &network.to_string(),
        ])
        .observe(time as f64);
}
pub fn set_blockchain_height_endpoint(
    url: &str,
    protocol: &Protocol,
    network: &Network,
    height: u64,
) {
    // retain only https://domain.tld
    let base_domain = get_base_url(url);
    BLOCKCHAIN_HEIGHT_ENDPOINT
        .with_label_values(&[&base_domain, &protocol.to_string(), &network.to_string()])
        .set(height as i64);
}

pub fn set_blockchain_metrics(
    protocol: &Protocol,
    network: &Network,
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

fn get_base_url(url: &str) -> String {
    let base_url = url
        .split('/')
        .nth(2)
        .unwrap_or("unknown")
        .split(':')
        .next()
        .unwrap_or("unknown")
        .to_string();
    // if base_url split . len > 2 => take last 2
    let mut base_url = base_url.split('.').collect::<Vec<&str>>();
    if base_url.len() > 2 {
        base_url = base_url[base_url.len() - 2..].to_vec();
    }
    base_url.join(".")
}

fn get_endpoint_status_metric(url: &str, protocol: &Protocol, network: &Network) -> bool {
    // retain only https://domain.tld
    let base_domain = get_base_url(url);
    let state = metrics::ENDPOINT_STATUS
        .with_label_values(&[&base_domain, &protocol.to_string(), &network.to_string()])
        .get();
    state == 1
}
pub fn set_endpoint_status_metric(url: &str, protocol: &Protocol, network: &Network, state: bool) {
    let m_state = get_endpoint_status_metric(url, protocol, network);
    if m_state == state {
        return;
    }
    let state = if state { 1 } else { 0 };
    let base_domain = get_base_url(url);
    metrics::ENDPOINT_STATUS
        .with_label_values(&[&base_domain, &protocol.to_string(), &network.to_string()])
        .set(state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prom_get_base_url() {
        assert_eq!(get_base_url("https://api.domain.tld"), "domain.tld");
        assert_eq!(get_base_url("https://api.domain.tld:1234"), "domain.tld");
        assert_eq!(
            get_base_url("https://api.domain.tld:1234/somethings"),
            "domain.tld"
        );
        assert_eq!(
            get_base_url("https://foo.bar.api.domain.tld:1234/somethings/else"),
            "domain.tld"
        );
    }
}
