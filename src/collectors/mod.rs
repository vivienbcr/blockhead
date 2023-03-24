use std::{error::Error, pin::Pin, time::Duration};

use futures::{future::join_all, Future};

use crate::{
    commons::blockchain::{Blockchain, self},
    configuration::{BitcoinOpts, NetworkName, ProtocolName, NetworkOptions},
    endpoints::Endpoint, prom,
};

pub async fn bitcoin(network_name: NetworkName, endpoints: BitcoinOpts) {
    info!(
        "Spawning collector for protocol: {:?}, network: {:?}",
        ProtocolName::Bitcoin,
        network_name
    );
    let network_opts = match endpoints.network_options{
        Some(mut n) => {
            n.init();
            n
        },
        None => NetworkOptions::default()
    };

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
        if rpcs.len() != 0 {
            futures_vec.extend(
                rpcs.iter_mut()
                    .filter(|r| r.available())
                    .map(|r| {
                        return r.parse_top_blocks( network_opts.head_length.unwrap());
                    })
                    .collect::<Vec<_>>(),
            );
        }
        match &mut blockstream {
            Some(b) => {
                if b.available() {
                    futures_vec.push(b.parse_top_blocks( network_opts.head_length.unwrap()));
                }
            }
            None => {}
        }

        let results = join_all(futures_vec).await;
        let results = results
            .into_iter()
            .filter_map(|r| match r {
                Ok(b) => Some(b),
                Err(_) => None
            })
            .collect::<Vec<_>>();
        if results.len() == 0 {
            error!("Bitcoin collector: no results from endpoints for network: {:?}", network_name);
            continue;
        }
        let mut best_chain = blockchain::get_highest_blockchain(results).unwrap();
        best_chain.sort();
        debug!("best_chain: {:?}", best_chain);
        prom::registry::set_blockchain_metrics(
            &best_chain.protocol,
            &best_chain.network,
            best_chain.height as i64,
            best_chain.blocks.last().unwrap().time as i64,
            best_chain.blocks.last().unwrap().txs as i64,
        );
    }
}
