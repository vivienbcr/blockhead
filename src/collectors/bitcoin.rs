// use std::time::Duration;

// use crate::{
//     configuration, endpoints::bitcoin_node::BitcoinNode,
//     endpoints::Endpoint, requests::client::ReqwestConfig, 
// };

// // global const len == 5
// const LEN: usize = 5; // TODO: from config

// pub async fn collect(url: String, network: String) {
//     let conf = configuration::CONFIGURATION.clone();
//     // get network from config
//     // let network = conf.protocols.get(PROTOCOL).unwrap().get(&network).unwrap();

//     let mut interval = tokio::time::interval(Duration::from_millis(1000)); // TODO: from config
//     loop {
//         interval.tick().await;
//         let reqwest_config = ReqwestConfig::new(url.to_string(), 10, Some(1));
//         let mainnet_node = BitcoinNode::new(reqwest_config.clone(), network.to_string());
//         let blockchain = mainnet_node.parse_top_blocks(3).await;

//         println!("Blockchain: {:?}", blockchain);
//     }
// }
