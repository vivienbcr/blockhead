use std::time::Duration;

use crate::{
    commons::blockchain,
    conf::{Network, NetworkAppOptions, Protocol, Provider},
    db::DATABASE,
    endpoints::ProviderActions,
    prom,
};

// pub mod bitcoin;
// pub mod ethereum;

pub async fn runner(
    protocol: Protocol,
    network: Network,
    providers: Vec<Provider>,
    net_opts: NetworkAppOptions,
) {
    info!(
        "Spawning collector for protocol: {:?}, network: {:?}, with providers: {:?}",
        &protocol.to_string(),
        &network.to_string(),
        providers.len()
    );
    let mut providers = providers;

    let mut interval =
        tokio::time::interval(Duration::from_secs(net_opts.tick_rate.unwrap() as u64));
    loop {
        // get all providers that implement ProviderActions
        let mut providers_d: Vec<Box<&mut dyn ProviderActions>> = Vec::new();
        for provider in providers.iter_mut() {
            let r = provider.as_mut_provider_actions();
            if r.is_some() {
                providers_d.push(Box::new(r.unwrap()));
            }
        }
        // batch all tasks
        let tasks = providers_d
            .iter_mut()
            .map(|p| p.parse_top_blocks(net_opts.head_length.unwrap()));
        let results = futures::future::join_all(tasks).await;
        // filter out errors
        let results = results
            .into_iter()
            .filter_map(|r| match r {
                Ok(b) => Some(b),
                Err(_) => None,
            })
            .collect::<Vec<_>>();
        if results.len() == 0 {
            error!(
                "{:?} collector: results for network: {:?}",
                &protocol.to_string(),
                &network.to_string()
            );
            interval.tick().await;
            continue;
        }
        let mut best_chain = blockchain::get_highest_blockchain(results).unwrap();
        best_chain.sort();
        debug!("best_chain: {:?}", best_chain);
        prom::registry::set_blockchain_metrics(
            protocol,
            network,
            best_chain.height as i64,
            best_chain.blocks.last().unwrap().time as i64,
            best_chain.blocks.last().unwrap().txs as i64,
        );
        let db = DATABASE.get().unwrap();
        let r = db.set_blockchain(&best_chain, &protocol, &network);
        match r {
            Ok(_) => {
                info!(
                    "Blockchain {} {} saved successfully",
                    protocol.to_string(),
                    &network.to_string()
                );
            }
            Err(e) => {
                error!(
                    "Error saving blockchain {} {}: {}",
                    protocol.to_string(),
                    network.to_string(),
                    e
                );
            }
        }
        interval.tick().await;
    }
}
