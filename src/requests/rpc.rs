use serde::{Deserialize, Serialize};
use super::client::ReqwestClient;
use crate::{prom::registry::track_status_code, configuration};
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcResponse<T>{
    pub jsonrpc: String,
    pub id: Option<u32>,
    pub result: Option<T>,
    pub error: Option<String>,
}
#[derive( Deserialize, Debug, Clone)]
pub enum JsonRpcParams {
    String (String),
    Number (u32),
}
impl Serialize for JsonRpcParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            JsonRpcParams::String(s) => serializer.serialize_str(s),
            JsonRpcParams::Number(n) => serializer.serialize_u32(*n),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcBody {
    pub jsonrpc: String,
    pub id: u32,
    pub method: String,
    pub params: Vec<JsonRpcParams>,
}
impl ReqwestClient {
    pub async fn rpc(&self, body: &JsonRpcBody, protocol : &str, network: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let url = self.config.url.clone().unwrap().clone();
        for i in 0..self.config.retry.unwrap_or(configuration::DEFAULT_ENDPOINT_RETRY) {
            let response = self
            .client
            .post(&url)
            .body(serde_json::to_string(&body).unwrap())
            .send()
            .await;
            if response.is_err() {
                debug!(
                    "Error: rpc request error, retrying in {} seconds, tries {} on {} ",
                    self.config.rate.unwrap_or(configuration::DEFAULT_ENDPOINT_REQUEST_RATE),
                    i,
                    self.config.retry.unwrap_or(configuration::DEFAULT_ENDPOINT_RETRY)
                );
                self.iddle().await;
                continue;
            }
            let response = response.unwrap();
            let status = response.status().as_u16();
            // TODO: in case of 429, we should implement exponential backoff
            track_status_code(&url, "POST",status, protocol, network );
            if status != 200 {
                debug!(
                    "Error: RPC {} status code {}, retrying in {} seconds, tries {} on {} ",
                    url,
                    status,
                    self.config.rate.unwrap_or(configuration::DEFAULT_ENDPOINT_REQUEST_RATE),
                    i,
                    self.config.retry.unwrap_or(configuration::DEFAULT_ENDPOINT_RETRY)
                );
                self.iddle().await;
                continue;
            }
            return Ok(response.text().await?);
        }
        return Err(format!("Error: RPC {} fail after {} retry",url,self.config.rate.unwrap_or(configuration::DEFAULT_ENDPOINT_RETRY)).into());
    }

    pub async fn get(&self, url: &str,protocol : &str, network: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let url = url.to_string();
        for i in 0..self.config.retry.unwrap_or(configuration::DEFAULT_ENDPOINT_RETRY) {
            let response = self.client.get(&url).send().await;
            if response.is_err() {

                debug!(
                    "Error: GET {} request error, retrying in {} seconds, tries {} on {} ",
                    url,
                    self.config.rate.unwrap_or(configuration::DEFAULT_ENDPOINT_REQUEST_RATE),
                    i,
                    self.config.retry.unwrap_or(configuration::DEFAULT_ENDPOINT_RETRY)
                );
                self.iddle().await;
                continue;
            }
            let response = response.unwrap();
            let status = response.status().as_u16();
            track_status_code(&url, "GET",status, protocol, network);
            if status != 200 {
                debug!(
                    "Error: GET {} status code {}, retrying in {} seconds, tries {} on {} ",
                    url,
                    status,
                    self.config.rate.unwrap_or(configuration::DEFAULT_ENDPOINT_REQUEST_RATE),
                    i,
                    self.config.retry.unwrap_or(configuration::DEFAULT_ENDPOINT_RETRY)
                );
                self.iddle().await;
                continue;
            }
            return Ok(response.text().await?);
        }
        return Err(format!("Error: GET {} fail after {} retry",url,self.config.rate.unwrap_or(configuration::DEFAULT_ENDPOINT_RETRY)).into());
    }
}