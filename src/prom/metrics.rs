use once_cell::sync::Lazy;
use prometheus::{
    register_histogram_vec, register_int_counter_vec, register_int_gauge_vec, HistogramVec,
    IntCounterVec, IntGaugeVec,
};

/**
 * HTTP request metrics
 */
pub static HTTP_REQUEST_CODE: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "http_request_code",
        "http request returns code",
        &["base_url", "status_code", "method", "proto", "network"]
    )
    .expect("metric can be created")
});
pub static HTTP_RESPONSE_TIME: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "http_response_time_ms",
        "Time to get response from endpoint in ms",
        &["base_url", "method", "proto", "network"],
        vec![
            0.5, 1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 450.0,
            500.0, 1000.0, 2500.0, 5000.0, 10000.0
        ]
    )
    .expect("metric can be created")
});
/**
 * Global blockchain metrics
 */
pub static BLOCKCHAIN_HEIGHT: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "blockchain_height",
        "Height of the blockchain",
        &["proto", "network"]
    )
    .expect("metric can be created")
});
pub static BLOCKCHAIN_HEIGHT_ENDPOINT: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "blockchain_height_endpoint",
        "Height of the blockchain per endpoint",
        &["endpoint", "proto", "network"]
    )
    .expect("metric can be created")
});
pub static BLOCKCHAIN_HEAD_TIMESTAMP: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "blockchain_head_timestamp",
        "Timestamp of the last block",
        &["proto", "network"]
    )
    .expect("metric can be created")
});
pub static BLOCKCHAIN_HEAD_TXS: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "blockchain_head_txs",
        "How many transactions are in the blockchain head",
        &["proto", "network"]
    )
    .expect("metric can be created")
});
