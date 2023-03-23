use std::{error::Error, pin::Pin, time::Duration};

use futures::{future::join_all, Future};

use crate::{
    commons::blockchain::Blockchain,
    configuration::{BitcoinEndpoints, NetworkName, ProtocolName},
    endpoints::Endpoint,
};

pub async fn bitcoin(network_name: NetworkName, endpoints: BitcoinEndpoints) {
    info!(
        "Spawning collector for protocol: {:?}, network: {:?}",
        ProtocolName::Bitcoin,
        network_name
    );
    let config = crate::configuration::CONFIGURATION.get().unwrap();
    let head_len = config.global.head_length;
    let str_name = network_name.to_string();

    let mut rpcs = endpoints.rpc.unwrap_or(Vec::new());
    rpcs.iter_mut().for_each(|r| r.init(&str_name).unwrap());

    let mut blockstream = match endpoints.blockstream {
        Some(mut b) => {
            b.init(&str_name).unwrap();
            Some(b)
        }
        None => None,
    };

    let mut interval = tokio::time::interval(Duration::from_millis(1000)); // TODO: from config
    loop {
        interval.tick().await;
        let mut futures_vec: Vec<
            Pin<Box<dyn Future<Output = Result<Blockchain, Box<dyn Error + Send + Sync>>> + Send>>,
        > = Vec::new();
        // let mut futures_vec: Vec<Pin<Box<dyn Future<Output = Result<Blockchain, Box<dyn Error + Send + Sync>>> + Send>>> = Vec::new();
        if rpcs.len() != 0 {
            futures_vec.extend(
                rpcs.iter_mut()
                    .filter(|r| r.available())
                    .map(|r| {
                        return r.parse_top_blocks( head_len);
                    })
                    .collect::<Vec<_>>(),
            );
        }
        match &mut blockstream {
            Some(b) => {
                if b.available() {
                    futures_vec.push(b.parse_top_blocks( head_len));
                }
            }
            None => {}
        }


        // if blockstream.is_some() && blockstream.as_ref().unwrap().available() {
        //     // let blockstream = blockstream.as_mut().unwrap();
        //     if blockstream.unwrap().available() {
        //         futures_vec.push(blockstream.unwrap().parse_top_blocks(&str_name, head_len));
        //     }
        // }

        let results = join_all(futures_vec).await;
        debug!("Results: {:?}", results);
        //TODO: Current blockchain finalization is not correct, it should be here after compare all blockchains returns by endpoints
    }
}
