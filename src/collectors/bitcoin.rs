use std::{error::Error, pin::Pin, time::Duration};

use futures::{future::join_all, Future};

use crate::{
    commons::blockchain::{self, Blockchain},
    configuration::{BitcoinOpts, EndpointActions, NetworkName, NetworkOptions, ProtocolName},
    db::DATABASE,
    endpoints::ProviderActions,
    prom,
};

pub async fn bitcoin(network_name: NetworkName, endpoints: BitcoinOpts) {
    info!(
        "Spawning collector for protocol: {:?}, network: {:?}",
        ProtocolName::Bitcoin,
        network_name
    );
    let network_opts = match endpoints.network_options {
        Some(mut n) => {
            n.init();
            n
        }
        None => NetworkOptions::default(),
    };

    let mut rpcs = endpoints.rpc.unwrap_or(Vec::new());
    let mut blockstream = endpoints.blockstream;
    let mut blockcypher = endpoints.blockcypher;
    if (rpcs.len() == 0) && (blockstream.is_none() && blockcypher.is_none()) {
        error!(
            "Bitcoin collector: no endpoints for network: {:?}",
            network_name.clone()
        );
        return;
    }

    let mut interval = tokio::time::interval(Duration::from_millis(5000)); // TODO: from config
    loop {
        let mut futures_vec: Vec<
            Pin<Box<dyn Future<Output = Result<Blockchain, Box<dyn Error + Send + Sync>>> + Send>>,
        > = Vec::new();
        if rpcs.len() != 0 {
            futures_vec.extend(
                rpcs.iter_mut()
                    .filter(|r| r.endpoint.available())
                    .map(|r| {
                        return r.parse_top_blocks(network_opts.head_length.unwrap());
                    })
                    .collect::<Vec<_>>(),
            );
        }
        match &mut blockstream {
            Some(b) => {
                if b.endpoint.available() {
                    futures_vec.push(b.parse_top_blocks(network_opts.head_length.unwrap()));
                }
            }
            None => {}
        }
        match &mut blockcypher {
            Some(b) => {
                if b.endpoint.available() {
                    futures_vec.push(b.parse_top_blocks(network_opts.head_length.unwrap()));
                }
            }
            None => {}
        }

        let results = join_all(futures_vec).await;
        let results = results
            .into_iter()
            .filter_map(|r| match r {
                Ok(b) => Some(b),
                Err(_) => None,
            })
            .collect::<Vec<_>>();
        if results.len() == 0 {
            error!(
                "Bitcoin collector: no results from endpoints for network: {:?}",
                network_name.clone()
            );
            continue;
        }
        let mut best_chain = blockchain::get_highest_blockchain(results).unwrap();
        best_chain.sort();
        debug!("best_chain: {:?}", best_chain);
        prom::registry::set_blockchain_metrics(
            ProtocolName::Bitcoin,
            network_name.clone(),
            best_chain.height as i64,
            best_chain.blocks.last().unwrap().time as i64,
            best_chain.blocks.last().unwrap().txs as i64,
        );
        let db = DATABASE.get().unwrap();

        let r = db.set_blockchain(&best_chain, &ProtocolName::Bitcoin, &network_name);
        match r {
            Ok(_) => {
                info!(
                    "Blockchain {} {} saved successfully",
                    ProtocolName::Bitcoin,
                    network_name
                );
            }
            Err(e) => {
                error!(
                    "Error saving blockchain {} {}: {}",
                    ProtocolName::Bitcoin,
                    network_name,
                    e
                );
            }
        }
        interval.tick().await;
    }
}
