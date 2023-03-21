use std::time::Duration;

use crate::configuration::{ProtocolName, NetworkName, ProtoEndpoints};

// pub mod bitcoin;


pub async fn collector(protocol : ProtocolName, network_name : NetworkName, endpoints : ProtoEndpoints ) {
    // TODO: Merge superseded endpoint options
    let mut interval = tokio::time::interval(Duration::from_millis(1000)); // TODO: from config
    loop {
        interval.tick().await;
        println!("Collector protocol: {:?}, networkName: {:?}, Endpoints: {:?}", protocol, network_name, endpoints);
    }
}