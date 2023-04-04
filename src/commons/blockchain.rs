use serde::{Deserialize, Serialize};

use crate::configuration::{NetworkName, ProtocolName};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Block {
    pub hash: String,
    pub height: u64,
    pub time: u64,
    pub txs: u64,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Blockchain {
    pub blocks: Vec<Block>,
    pub height: u64,
}
impl Blockchain {
    pub fn new(blocks: Option<Vec<Block>>) -> Blockchain {
        let b = match blocks {
            Some(b) => b,
            None => Vec::new(),
        };
        Blockchain {
            blocks: b,
            height: 0,
        }
    }
    pub fn add_block(&mut self, block: Block) {
        if self.height < block.height {
            self.height = block.height;
        }
        self.blocks.push(block);
    }
    pub fn sort(&mut self) {
        if self.blocks.len() == 0 {
            return;
        }
        self.blocks.sort_by(|a, b| b.height.cmp(&a.height));
        if self.blocks.len() > 0 {
            self.height = self.blocks.first().unwrap().height;
        }
    }
}

pub fn get_highest_blockchain(blockchains: Vec<Blockchain>) -> Option<Blockchain> {
    match blockchains.len() {
        0 => None,
        _ => Some(
            blockchains
                .iter()
                .filter(|b| b.height > 0)
                .max_by(|a, b| a.height.cmp(&b.height))
                .unwrap()
                .clone(),
        ),
    }
}
