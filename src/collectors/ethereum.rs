use futures::{future::join_all, Future};

use crate::{
    commons::blockchain::{self, Blockchain},
    configuration::{EndpointActions, EthereumOpts, NetworkName, ProtocolName},
    db::DATABASE,
    endpoints::ProviderActions,
    prom,
};
use std::{error::Error, pin::Pin, time::Duration};

pub async fn ethereum(network_name: NetworkName, endpoints: EthereumOpts) {
    info!(
        "Spawning collector for protocol: {:?}, network: {:?}",
        ProtocolName::Ethereum,
        network_name
    );
    println!(
        "Endpoints for ethereum {:?}",
        &endpoints.network_options.clone()
    );
    let network_opts = endpoints.network_options.clone().unwrap();

    let mut rpcs = endpoints.rpc.unwrap_or(Vec::new());
    if rpcs.len() == 0 {
        error!(
            "Ethereum collector: no endpoints for network: {:?}",
            network_name.clone()
        );
        return;
    }

    let mut interval = tokio::time::interval(Duration::from_secs(network_opts.tick_rate as u64)); // FIXME: from config
    loop {
        let mut futures_vec: Vec<
            Pin<Box<dyn Future<Output = Result<Blockchain, Box<dyn Error + Send + Sync>>> + Send>>,
        > = Vec::new();
        if rpcs.len() != 0 {
            println!("rpcs: {:?}", rpcs.len());
            futures_vec.extend(
                rpcs.iter_mut()
                    .filter(|r| {
                        println!("Filter r: {:?}", r);
                        r.endpoint.available()
                    })
                    .map(|r| {
                        return r.parse_top_blocks(network_opts.head_length.unwrap());
                    })
                    .collect::<Vec<_>>(),
            );
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
                "Ethereum collector: no results from endpoints for network: {:?}",
                network_name.clone()
            );
            interval.tick().await;
            continue;
        }
        let mut best_chain = blockchain::get_highest_blockchain(results).unwrap();
        best_chain.sort();
        debug!("best_chain: {:?}", best_chain);
        prom::registry::set_blockchain_metrics(
            ProtocolName::Ethereum,
            network_name.clone(),
            best_chain.height as i64,
            best_chain.blocks.last().unwrap().time as i64,
            best_chain.blocks.last().unwrap().txs as i64,
        );
        let db = DATABASE.get().unwrap();

        let r = db.set_blockchain(&best_chain, &ProtocolName::Ethereum, &network_name);
        match r {
            Ok(_) => {
                info!(
                    "Blockchain {} {} saved successfully",
                    ProtocolName::Ethereum,
                    network_name
                );
            }
            Err(e) => {
                error!(
                    "Error saving blockchain {} {}: {}",
                    ProtocolName::Ethereum,
                    network_name,
                    e
                );
            }
        }
        interval.tick().await;
    }
}
