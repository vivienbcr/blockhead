use super::client::ReqwestClient;
use crate::{prom::registry::track_response_time, prom::registry::track_status_code};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Error;
pub const JSON_RPC_VER: &str = "2.0";
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: Option<u32>,
    pub result: Option<T>,
    pub error: Option<String>,
}
#[derive(Deserialize, Debug, Clone)]
pub enum JsonRpcParams {
    String(String),
    Number(u32),
    Bool(bool),
}
impl Serialize for JsonRpcParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            JsonRpcParams::String(s) => serializer.serialize_str(s),
            JsonRpcParams::Number(n) => serializer.serialize_u32(*n),
            JsonRpcParams::Bool(b) => serializer.serialize_bool(*b),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcReq {
    pub jsonrpc: String,
    pub id: u32,
    pub method: String,
    pub params: Vec<JsonRpcParams>,
}
#[derive(Deserialize, Debug, Clone)]
pub enum JsonRpcReqBody {
    Single(JsonRpcReq),
    Batch(Vec<JsonRpcReq>),
}
// implement custom serialization for JsonRpcReqBody
impl Serialize for JsonRpcReqBody {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            JsonRpcReqBody::Single(req) => req.serialize(serializer),
            JsonRpcReqBody::Batch(reqs) => reqs.serialize(serializer),
        }
    }
}

impl ReqwestClient {
    pub async fn rpc<T: DeserializeOwned>(
        &self,
        body: &JsonRpcReqBody,
        protocol: &str,
        network: &str,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let b = serde_json::to_string(&body).unwrap();
        debug!("RPC request: {}", &b);
        let url = self.config.url.clone().unwrap().clone();
        for i in 0..self.config.retry {
            let time_start = std::time::Instant::now();
            let response = self
                .client
                .post(&url)
                .body(b.clone())
                .header("Content-Type", "application/json")
                .send()
                .await;
            let time_duration = time_start.elapsed().as_millis();
            if response.is_err() {
                debug!(
                    "Error: rpc request {} error, retrying in {} seconds, tries {} on {} ",
                    &b, self.config.rate, i, self.config.retry
                );
                self.iddle().await;
                continue;
            }
            let response = response.unwrap();
            let status = response.status().as_u16();
            track_status_code(&url, "POST", status, protocol, network);
            if status != 200 {
                debug!(
                    "Error: RPC {} status code {}, retrying in {} seconds, tries {} on {}, body: {}",
                    url,
                    status,
                    self.config
                        .rate
                        ,
                    i,
                    self.config
                        .retry
                        ,
                    &b
                );
                self.iddle().await;
                continue;
            }
            trace!("RPC {} response: {:?}", url, response);
            let txt = response.text().await;
            trace!("RPC {} response text: {:?}", url, txt);
            track_response_time(&url, "POST", protocol, network, time_duration);
            debug!("RPC {} OK", url);
            let r: Result<T, Error> = serde_json::from_str(&txt?);
            match r {
                Ok(r) => return Ok(r),
                Err(e) => {
                    error!("Error: RPC decode {} response error: {}", url, e);
                    return Err(e.into());
                }
            }
        }
        return Err(format!("Error: RPC {} fail after {} retry", url, self.config.rate).into());
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        url: &str,
        protocol: &str,
        network: &str,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let url = url.to_string();
        trace!("GET {} request", url);
        for i in 0..self.config.retry {
            let time_start = std::time::Instant::now();
            let response = self.client.get(&url).send().await;
            let time_duration = time_start.elapsed().as_millis();
            if response.is_err() {
                debug!(
                    "Error: GET {} request error, retrying in {} seconds, tries {} on {} ",
                    url, self.config.rate, i, self.config.retry
                );
                self.iddle().await;
                continue;
            }
            let response = response.unwrap();
            let status = response.status().as_u16();
            track_status_code(&url, "GET", status, protocol, network);
            if status != 200 {
                debug!(
                    "Error: GET {} status code {}, retrying in {} seconds, tries {} on {} ",
                    url, status, self.config.rate, i, self.config.retry
                );
                self.iddle().await;
                continue;
            }
            track_response_time(&url, "GET", protocol, network, time_duration);
            let r: T = serde_json::from_str(&response.text().await?)?;
            return Ok(r);
        }
        return Err(format!("Error: GET {} fail after {} retry", url, self.config.rate).into());
    }
}
