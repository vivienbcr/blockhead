use std::{time::Duration, clone};

use futures::future::join_all;

use crate::{configuration::{ProtocolName, NetworkName,  BitcoinEndpoints}, endpoints::{ Endpoint}};

// pub mod bitcoin;


pub async fn bitcoin( network_name : NetworkName, endpoints : BitcoinEndpoints ) {
    info!("Spawning collector for protocol: {:?}, network: {:?}", ProtocolName::Bitcoin, network_name);
    let str_name = network_name.to_string();

    let mut rpcs = endpoints.rpc.unwrap();
    rpcs.iter_mut().for_each(|r| r.init().unwrap());

    let mut interval = tokio::time::interval(Duration::from_millis(1000)); // TODO: from config
    loop {
        interval.tick().await;
        let futures_vec = rpcs.iter_mut().filter(|r| r.available()).map(|r| {
            return r.parse_top_blocks(&str_name, 3);
        }).collect::<Vec<_>>();

        let results = join_all(futures_vec).await;
        debug!("Results: {:?}", results);

    }
}