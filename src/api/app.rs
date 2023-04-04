use std::collections::HashMap;

use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use serde_json::to_string;

use crate::{
    commons::blockchain::Blockchain,
    configuration::{get_enabled_protocol_network, NetworkName, ProtocolName},
    db::DATABASE,
};

#[derive(Deserialize, Debug)]
struct BlockchainRouteParams {
    protocol: Option<ProtocolName>,
    network: Option<NetworkName>,
}

type BlockchainRes = HashMap<ProtocolName, HashMap<NetworkName, Blockchain>>;

#[get("/")]
async fn blockchain_handler(params: web::Query<BlockchainRouteParams>) -> HttpResponse {
    info!("Blockchain route called with params: {:?}", params);
    let db = DATABASE.get().unwrap();
    let proto_net = get_enabled_protocol_network();

    if proto_net.is_empty() {
        return HttpResponse::Ok().body("Welcome to Blockhead!");
    }
    // if proto param is
    if params.protocol.is_some() {
        let param_protocol = params.protocol.clone().unwrap();
        debug!("Protocol param is {}", param_protocol.clone());
        if !proto_net.contains_key(&param_protocol) {
            return HttpResponse::BadRequest()
                .body(format!("{} protocol not found", param_protocol.clone()));
        }
        if params.network.is_some() {
            // network should be in proto_net for that protocol
            let param_network = params.network.clone().unwrap();
            let proto_net = proto_net.get(&param_protocol).unwrap().clone();
            if !proto_net.contains(&param_network) {
                return HttpResponse::BadRequest().body(format!(
                    "{} network not found for {} protocol",
                    param_network, param_protocol
                ));
            }
            let response = db.get_blockchain(&param_protocol, &param_network).unwrap();
            // should return a BlockchainRes
            let mut proto: BlockchainRes = HashMap::new();
            let mut data = HashMap::new();
            data.insert(param_network, response);
            proto.insert(param_protocol, data);
            return HttpResponse::Ok()
                .content_type("application/json")
                .body(to_string(&proto).unwrap());
        }
        let mut data: BlockchainRes = BlockchainRes::new();
        let available_networks = proto_net.get(&param_protocol).unwrap().clone();
        for network in available_networks {
            debug!(
                "Getting Database {} network for {} protocol",
                network,
                param_protocol.clone()
            );
            //FIXME: if database return one empty response, handler crash
            let response = db.get_blockchain(&param_protocol, &network).unwrap();
            data.entry(param_protocol.clone())
                .or_insert(HashMap::new())
                .insert(network, response);
        }
    }

    let mut data = BlockchainRes::new();
    for (protocol, networks) in proto_net {
        for network in networks {
            let response = db.get_blockchain(&protocol, &network).unwrap();
            data.entry(protocol.clone())
                .or_insert(HashMap::new())
                .insert(network, response);
        }
    }
    HttpResponse::Ok()
        .content_type("application/json")
        .body(to_string(&data).unwrap())
}
