use once_cell::sync::Lazy;
use prometheus::{register_int_counter_vec, register_int_gauge_vec, IntCounterVec, IntGaugeVec};

/**
 * HTTP request metrics
 */
pub static HTTP_REQUEST_CODE_200: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "http_request_code_200",
        "http request returns code 200",
        &["base_url","method", "proto", "network"]
    )
    .expect("metric can be created")
});
pub static HTTP_REQUEST_CODE_400: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "http_request_code_400",
        "http request returns code 400",
        &["base_url","method", "proto", "network"]
    )
    .expect("metric can be created")
});
pub static HTTP_REQUEST_CODE_500: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "http_request_code_500",
        "http request returns code 500",
        &["base_url","method", "proto", "network"]
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
