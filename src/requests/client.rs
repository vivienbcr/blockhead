use reqwest::Client;

#[derive(Debug,Clone)]
pub struct ReqwestConfig {
    pub base_url: String,
    pub retry: u32,
    pub hiddle: Option<u64>,
}
impl ReqwestConfig {
    pub fn new(base_url: String, retry: u32, hiddle: Option<u64>) -> ReqwestConfig {
        ReqwestConfig {
            base_url,
            retry,
            hiddle,
        }
    }
}

pub struct ReqwestClient {
    pub client: Client,
    pub config: ReqwestConfig,
}

impl ReqwestClient {
    pub fn new(mut config: ReqwestConfig) -> ReqwestClient {
        if config.hiddle.is_none() {
            config.hiddle = Some(1);
        }

        ReqwestClient {
            client: Client::new(),
            config,
        }
    }
    pub async fn iddle(&self) {
        tokio::time::sleep(tokio::time::Duration::from_secs(
            self.config.hiddle.unwrap_or(1),
        ))
        .await;
    }
}
