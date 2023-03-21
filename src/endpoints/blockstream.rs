use serde::{Deserialize, Serialize};

use crate::{configuration::EndpointOptions, requests::client::ReqwestClient};

#[derive(Deserialize, Serialize,Debug,Clone)]
pub struct BlockstreamEndpoint {
    pub url: String,
    pub options: Option<EndpointOptions>,
    #[serde(skip)]
    pub reqwest: Option<ReqwestClient>,
    // pub network: String,
}