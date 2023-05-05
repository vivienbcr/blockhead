use crate::conf::EndpointOptions;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ReqwestClient {
    // pub client: Client,
    pub config: EndpointOptions,
    pub last_request: u64,
}

impl ReqwestClient {
    pub fn new(config: EndpointOptions) -> ReqwestClient {
        ReqwestClient {
            config,
            last_request: 0,
        }
    }
    pub async fn iddle(&self) {
        tokio::time::sleep(tokio::time::Duration::from_secs(self.config.delay as u64)).await;
    }
    pub fn set_last_request(&mut self) {
        self.last_request = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
    }
    pub fn available(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        let diff = now - self.last_request;
        if diff < self.config.rate as u64 {
            debug!(
                "Rate limit reached for {} ({}s)",
                self.config.url.clone().unwrap_or("UNSET_URL".to_string()),
                diff
            );
            return false;
        }
        true
    }
}
