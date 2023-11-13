use std::collections::HashMap;

use actix_web::{get, web, HttpResponse};
use serde_json::to_string;

use crate::{
    commons::blockchain::Blockchain,
    conf::{get_enabled_protocol_network, Network, Protocol},
    db::DATABASE,
};

type BlockchainRes = HashMap<Protocol, HashMap<Network, Blockchain>>;

#[get("/ping")]
async fn ping_handler() -> HttpResponse {
    HttpResponse::Ok().body("pong")
}
#[get("/protocols/{protocol}/{network}")]
async fn protocol_net_handler(params: web::Path<(Protocol, Network)>) -> HttpResponse {
    let db = match DATABASE.get() {
        Some(db) => db,
        None => return HttpResponse::InternalServerError().body("Database not initialized"),
    };
    let (protocol, network) = params.into_inner();
    let response = db.get_blockchain(&protocol, &network).unwrap();
    HttpResponse::Ok()
        .content_type("application/json")
        .body(to_string(&response).unwrap())
}
#[get("/protocols/{protocol}")]
async fn protocol_handler(params: web::Path<Protocol>) -> HttpResponse {
    let db = match DATABASE.get() {
        Some(db) => db,
        None => return HttpResponse::InternalServerError().body("Database not initialized"),
    };

    let protocol = params.into_inner();

    // Get the enabled protocol network
    let proto_net = match get_enabled_protocol_network() {
        Some(protocols_net_list) => protocols_net_list,
        None => return HttpResponse::InternalServerError().body("No protocol enabled"),
    };
    let proto = match proto_net.get(&protocol) {
        Some(proto) => proto,
        None => {
            return HttpResponse::BadRequest()
                .body(format!("{:?} protocol not found", protocol.clone()))
        }
    };
    let mut data = HashMap::new();
    for network in proto {
        debug!(
            "Getting Database {:?} network for {:?} protocol",
            network,
            protocol.clone()
        );
        let response = db.get_blockchain(&protocol, network);
        match response {
            Ok(response) => {
                data.insert(network, response);
            }
            Err(e) => {
                error!(
                    "Database req {} {} return error: {}",
                    &protocol.to_string(),
                    &network.to_string(),
                    e
                );
                return HttpResponse::InternalServerError().body(format!(
                    "No data found for {:?} protocol and {:?} network",
                    protocol.clone(),
                    network.clone()
                ));
            }
        }
    }
    HttpResponse::Ok()
        .content_type("application/json")
        .body(to_string(&data).unwrap())
}

#[get("/protocols")]
async fn protocols_handler() -> HttpResponse {
    let db = match DATABASE.get() {
        Some(db) => db,
        None => return HttpResponse::InternalServerError().body("Database not initialized"),
    };

    let proto_net = match get_enabled_protocol_network() {
        Some(protocols_net_list) => protocols_net_list,
        None => return HttpResponse::InternalServerError().body("No protocol enabled"),
    };

    let mut data = BlockchainRes::new();
    for (protocol, networks) in proto_net {
        for network in networks {
            let response = db.get_blockchain(&protocol, &network).unwrap();
            data.entry(protocol)
                .or_insert(HashMap::new())
                .insert(network, response);
        }
    }
    HttpResponse::Ok()
        .content_type("application/json")
        .body(to_string(&data).unwrap())
}
