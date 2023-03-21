use reqwest::Client;

use crate::configuration::EndpointOptions;

#[derive(Debug,Clone)]
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
        tokio::time::sleep(tokio::time::Duration::from_secs(
            match self.config.rate {
                Some(rate) => rate as u64,
                None => 1,
            }
        ))
        .await;
    }
}
