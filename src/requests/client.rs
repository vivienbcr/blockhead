use reqwest::Client;

use crate::conf::EndpointOptions;

#[derive(Debug, Clone)]
pub struct ReqwestClient {
    pub client: Client,
    pub config: EndpointOptions,
}

impl ReqwestClient {
    pub fn new(config: EndpointOptions) -> ReqwestClient {
        ReqwestClient {
            client: Client::new(),
            config,
        }
    }
    pub async fn iddle(&self) {
        tokio::time::sleep(tokio::time::Duration::from_secs(self.config.delay as u64)).await;
    }
}
