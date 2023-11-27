use std::str::FromStr;

use super::client::ReqwestClient;
use crate::{
    conf::{Network, Protocol},
    prom::registry::track_status_code,
    prom::registry::{set_endpoint_status_metric, track_response_time},
};
use reqwest::{
    header::{HeaderMap, HeaderName},
    Client, StatusCode,
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
    SerdeValue(serde_json::Value),
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
            JsonRpcParams::SerdeValue(v) => v.serialize(serializer),
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
    fn get_timout(&self) -> tokio::time::Duration {
        tokio::time::Duration::from_secs(self.config.timeout as u64)
    }
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
                    match v.parse() {
                        Ok(v) => {
                            headers.insert(key, v);
                        }
                        Err(e) => {
                            error!("Error parsing header value: {}", e);
                            continue;
                        }
                    };
                }
            }
            None => {}
        }
        match "application/json".parse() {
            Ok(v) => {
                headers.insert("Content-Type", v);
            }
            Err(e) => {
                error!("Error parsing header value: {}", e);
            }
        }

        headers
    }
    pub async fn rpc<T: DeserializeOwned>(
        &mut self,
        body: &JsonRpcReqBody,
        protocol: &Protocol,
        network: &Network,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let b = serde_json::to_string(&body);
        let b = match b {
            Ok(b) => b,
            Err(e) => {
                return Err(Box::new(RequestError::DeserializeRequestError(e)));
            }
        };
        trace!("RPC request: {}", &b);
        let url = self.config.url.clone();
        let url = match url {
            Some(url) => url,
            None => {
                return Err(Box::new(RequestError::UndefinedUrl));
            }
        };
        let mut c = 0;
        for i in 0..self.config.retry {
            c += i; // count for logger
            let time_start = std::time::Instant::now();
            let client = Client::new();
            let headers = self.get_headers();
            let request = client
                .post(&url)
                .body(b.clone())
                .headers(headers)
                .timeout(self.get_timout());
            let request = match self.config.basic_auth.clone() {
                Some(auth) => request.basic_auth(auth.username, Some(auth.password)),
                None => request,
            };
            let response = request.send().await;
            let time_duration = time_start.elapsed().as_millis();
            self.set_last_request();
            let response = match response {
                Ok(response) => response,
                Err(e) => {
                    error!(
                        "rpc {} request {} return error code {:?} source: {}, retrying in {} seconds, tries {} on {} ",
                        &url, &b,e.status(),e.to_string(),  self.config.delay, i, self.config.retry
                    );
                    // As we wait for a response to continue to process data, iter on timeout will take too much time
                    if e.is_timeout() {
                        error!("Timeout detected skip this requests...");
                        track_status_code(&url, &self.alias, "POST", 504, protocol, network);
                        return Err(Box::new(e));
                    }
                    track_status_code(
                        &url,
                        &self.alias,
                        "POST",
                        e.status()
                            .unwrap_or(StatusCode::SERVICE_UNAVAILABLE)
                            .as_u16(),
                        protocol,
                        network,
                    );
                    self.iddle().await;
                    continue;
                }
            };

            let status = response.status().as_u16();
            debug!(
                "POST {} {} {} {}ms",
                &url, &self.alias, status, time_duration
            );
            track_status_code(&url, &self.alias, "POST", status, protocol, network);
            if status == StatusCode::TOO_MANY_REQUESTS.as_u16() {
                error!("rpc {} return too many request, skipping this request", url,);
                return Err(Box::new(RequestError::EndpointReachRateLimit(
                    url.to_string(),
                )));
            }
            if status != StatusCode::OK.as_u16() {
                error!(
                    "rpc {} status code {}, retrying in {} seconds, tries {} on {}, body: {}",
                    url, status, self.config.delay, i, self.config.retry, &b
                );
                self.iddle().await;
                continue;
            }
            let txt = response.text().await?;
            set_endpoint_status_metric(&url, &self.alias, protocol, network, true);
            track_response_time(
                &url,
                &self.alias,
                &reqwest::Method::POST,
                protocol,
                network,
                time_duration,
            );
            let r: Result<T, Error> = serde_json::from_str(&txt);
            match r {
                Ok(r) => return Ok(r),
                Err(e) => {
                    error!("rpc decode {} response error: {}\nraw : {}", url, e, &txt);
                    return Err(e.into());
                }
            }
        }
        // After all retry, set endpoint down and return error
        set_endpoint_status_metric(&url, &self.alias, protocol, network, false);
        Err(format!("rpc {} fail after {} retry", &url, &c).into())
    }

    pub async fn run_request<T: DeserializeOwned>(
        &mut self,
        method: reqwest::Method,
        body: Option<serde_json::Value>,
        url: &str,
        protocol: &Protocol,
        network: &Network,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let url = url.to_string();
        let mut c = 0;
        for i in 0..self.config.retry {
            c += i;
            let time_start = std::time::Instant::now();
            let client = Client::new();
            let headers = self.get_headers();
            let request = client
                .request(method.clone(), &url)
                .headers(headers)
                .timeout(self.get_timout());
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
            self.set_last_request();
            let time_duration = time_start.elapsed().as_millis();
            let response = match response {
                Ok(response) => response,
                Err(e) => {
                    error!(
                        "Error: {} {} request error, retrying in {} seconds, tries {} on {} : {} ",
                        &method, url, self.config.delay, i, self.config.retry, e
                    );
                    self.iddle().await;
                    continue;
                }
            };
            let status = response.status().as_u16();
            track_status_code(
                &url,
                &self.alias,
                &format!("{}", &method),
                status,
                protocol,
                network,
            );

            if status != 200 {
                error!(
                    "{} {} status code {}, retrying in {} seconds, tries {} on {} ",
                    &method, url, status, self.config.delay, i, self.config.retry
                );
                self.iddle().await;
                continue;
            }
            track_response_time(&url, &self.alias, &method, protocol, network, time_duration);
            let r_txt = response.text().await;
            let r_txt = match r_txt {
                Ok(r_txt) => r_txt,
                Err(e) => {
                    error!(
                        "{} {} response error: {}, retrying in {} seconds, tries {} on {} ",
                        &method, url, e, self.config.delay, i, self.config.retry
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
                        "{} {} response decode error: {}, retrying in {} seconds, tries {} on {}\nraw: {} ",
                        &method, url, e, self.config.delay, i, self.config.retry, &r_txt
                    );
                    self.iddle().await;
                    continue;
                }
            };
            return Ok(r);
        }
        Err(format!("{} {} fail after {} retry", &method, &url, &c).into())
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
                &Protocol::Tezos,
                &String::from("mainnet"),
            )
            .await;
        assert!(res.is_ok());
    }
}
