use std::{time::Duration, clone};

use crate::{configuration::{ProtocolName, NetworkName, ProtoEndpoints, BitcoinEndpoints, self}, endpoints::{blockstream, bitcoin_node::{self, BitcoinNode}, Endpoint}};

// pub mod bitcoin;


pub async fn bitcoin( network_name : NetworkName, endpoints : BitcoinEndpoints ) {
    let str_name = network_name.to_string();
    // Setups rpcs co
    let mut rpcs = endpoints.rpc.unwrap();
    rpcs.iter_mut().for_each(|r| r.init().unwrap());

    // for rpc in rpcs.iter_mut() {
    //    rpc.init().await; 
    // }

    // TODO: Merge superseded endpoint options
    let mut interval = tokio::time::interval(Duration::from_millis(1000)); // TODO: from config
    loop {
        interval.tick().await;
        for i in 0..rpcs.len() {
            let rpc = rpcs.get_mut(i).unwrap();
            if rpc.available() {
                let blockchain = rpc.parse_top_blocks(&str_name, 3).await;
                println!("Blockchain: {:?}", blockchain);
            }
        }

        // println!("Collector protocol: Bitcoin, networkName: {:?}, Endpoints: {:?}",  network_name, endpoints);
    }
}