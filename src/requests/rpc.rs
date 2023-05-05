use std::str::FromStr;

use super::client::ReqwestClient;
use crate::{prom::registry::track_response_time, prom::registry::track_status_code};
use reqwest::{
    header::{HeaderMap, HeaderName},
    Client,
};
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
#[derive(Debug)]
pub enum RequestError {
    UndefinedUrl,
    EndpointReachRateLimit(String),
    DeserializeRequestError(Error),
}
impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RequestError::UndefinedUrl => write!(f, "Undefined url"),
            RequestError::EndpointReachRateLimit(url) => {
                write!(f, "Endpoint reach internal rate limit: {}", url)
            }
            RequestError::DeserializeRequestError(e) => {
                write!(f, "Deserialize request error: {}", e)
            }
        }
    }
}
impl std::error::Error for RequestError {}

impl ReqwestClient {
    fn get_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        match &self.config.headers {
            Some(h) => {
                for (k, v) in h {
                    let key = match HeaderName::from_str(k) {
                        Ok(key) => key,
                        Err(e) => {
                            error!("Error parsing header name: {}", e);
                            continue;
                        }
                    };
                    headers.insert(key, v.parse().unwrap());
                }
            }
            None => {}
        }
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }
    pub async fn rpc<T: DeserializeOwned>(
        &mut self,
        body: &JsonRpcReqBody,
        protocol: &str,
        network: &str,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        if !self.available() {
            return Err(Box::new(RequestError::EndpointReachRateLimit(
                self.config.url.clone().unwrap_or("UNSET_URL".to_string()),
            )));
        }
        let b = serde_json::to_string(&body);
        let b = match b {
            Ok(b) => b,
            Err(e) => {
                return Err(Box::new(RequestError::DeserializeRequestError(e)));
            }
        };
        debug!("RPC request: {}", &b);
        let url = self.config.url.clone();
        let url = match url {
            Some(url) => url,
            None => {
                return Err(Box::new(RequestError::UndefinedUrl));
            }
        };

        for i in 0..self.config.retry {
            let time_start = std::time::Instant::now();
            let client = Client::new();
            let headers = self.get_headers();
            let request = client.post(&url).body(b.clone()).headers(headers);
            let request = match self.config.basic_auth.clone() {
                Some(auth) => request.basic_auth(auth.username, Some(auth.password)),
                None => request,
            };
            let response = request.send().await;
            let time_duration = time_start.elapsed().as_millis();
            if response.is_err() {
                debug!(
                    "Error: rpc request {} error, retrying in {} seconds, tries {} on {} ",
                    &b, self.config.rate, i, self.config.retry
                );
                self.iddle().await;
                continue;
            }
            let response = response?;
            let status = response.status().as_u16();
            track_status_code(&url, "POST", status, protocol, network);
            if status != 200 {
                error!(
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
            let txt = response.text().await?;
            track_response_time(
                &url,
                &reqwest::Method::POST,
                protocol,
                network,
                time_duration,
            );
            let r: Result<T, Error> = serde_json::from_str(&txt);
            match r {
                Ok(r) => return Ok(r),
                Err(e) => {
                    error!(
                        "Error: RPC decode {} response error: {}\nraw : {}",
                        url, e, &txt
                    );
                    return Err(e.into());
                }
            }
        }
        return Err(format!("Error: RPC {} fail after {} retry", url, self.config.rate).into());
    }

    pub async fn run_request<T: DeserializeOwned>(
        &mut self,
        method: reqwest::Method,
        body: Option<serde_json::Value>,
        url: &str,
        protocol: &str,
        network: &str,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        if !self.available() {
            return Err(Box::new(RequestError::EndpointReachRateLimit(
                self.config.url.clone().unwrap_or("UNSET_URL".to_string()),
            )));
        }

        let url = url.to_string();
        for i in 0..self.config.retry {
            let time_start = std::time::Instant::now();
            let client = Client::new();
            let headers = self.get_headers();
            let request = client.request(method.clone(), &url).headers(headers);
            let request = match self.config.basic_auth.clone() {
                Some(auth) => request.basic_auth(auth.username, Some(auth.password)),
                None => request,
            };
            let request = match body.clone() {
                Some(body) => request.body(body.to_string()),
                None => request,
            };
            trace!("{} {} request", &method, url);
            let response: Result<reqwest::Response, reqwest::Error> = request.send().await;
            let time_duration = time_start.elapsed().as_millis();
            let response = match response {
                Ok(response) => response,
                Err(e) => {
                    error!(
                        "Error: {} {} request error, retrying in {} seconds, tries {} on {} : {} ",
                        &method, url, self.config.rate, i, self.config.retry, e
                    );
                    self.iddle().await;
                    continue;
                }
            };
            let status = response.status().as_u16();
            track_status_code(&url, &format!("{}", &method), status, protocol, network);

            if status != 200 {
                error!(
                    "Error: '{} {} status code {}, retrying in {} seconds, tries {} on {} ",
                    &method, url, status, self.config.rate, i, self.config.retry
                );
                self.iddle().await;
                continue;
            }
            track_response_time(&url, &method, protocol, network, time_duration);
            let r_txt = response.text().await;
            let r_txt = match r_txt {
                Ok(r_txt) => r_txt,
                Err(e) => {
                    error!(
                        "Error: {} {} response error: {}, retrying in {} seconds, tries {} on {} ",
                        &method, url, e, self.config.rate, i, self.config.retry
                    );
                    self.iddle().await;
                    continue;
                }
            };
            let r: Result<T, serde_json::Error> = serde_json::from_str(&r_txt);
            let r = match r {
                Ok(r) => r,
                Err(e) => {
                    debug!(
                        "Error: {} {} response decode error: {}, retrying in {} seconds, tries {} on {}\nraw: {} ",
                        &method, url, e, self.config.rate, i, self.config.retry, &r_txt
                    );
                    self.iddle().await;
                    continue;
                }
            };
            return Ok(r);
        }
        return Err(format!(
            "Error: {} {} fail after {} retry",
            &method, url, self.config.rate
        )
        .into());
    }
}

#[cfg(test)]
mod tests {

    extern crate env_logger;
    use super::*;
    use crate::{
        conf::{BasicAuth, EndpointOptions},
        tests,
    };
    #[tokio::test]
    async fn request_basic_auth_get() {
        tests::setup();
        let url = "http://httpbin.org/basic-auth/foo/bar";
        let mut endpoint_options: EndpointOptions = Default::default();
        endpoint_options.url = Some(url.to_string());
        endpoint_options.basic_auth = Some(BasicAuth {
            username: "foo".to_string(),
            password: "bar".to_string(),
        });
        let mut client = ReqwestClient::new(endpoint_options);
        let res = client
            .run_request::<serde_json::Value>(
                reqwest::Method::GET,
                None,
                url,
                "protocol",
                "network",
            )
            .await;
        assert!(res.is_ok());
    }
}
