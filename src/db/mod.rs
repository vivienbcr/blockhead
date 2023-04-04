use std::error::Error;

use once_cell::sync::OnceCell;
use redb::{Database, ReadableTable, TableDefinition};

use std::io;

use crate::{
    commons::blockchain::{self, Block},
    configuration::{NetworkName, ProtocolName, CONFIGURATION},
};
const TABLE: TableDefinition<&str, &str> = TableDefinition::new("blockchain");
pub static DATABASE: OnceCell<Redb> = OnceCell::new();
#[derive(Debug)]
pub struct Redb {
    db: Database,
}
impl Redb {
    pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Redb::new");
        let db = Database::open("bh_db.redb");
        match db {
            Ok(db) => {
                info!("Redb::new() db is ok");
                let db = db;
                DATABASE.set(Redb { db: db }).unwrap();
                Ok(())
            }
            Err(e) => {
                match &e {
                    redb::Error::Io(io_error) => {
                        match io_error.kind() {
                            io::ErrorKind::NotFound => {
                                info!("Redb db is not found, create new one");
                                let db = Database::create("bh.redb");
                                match db {
                                    Ok(db) => {
                                        info!("Redb db is created");
                                        let rdb = Redb { db: db };
                                        // it seem if we don't insert a first data, db will not be able to be reopen
                                        rdb.set("keep", "1")?;
                                        DATABASE.set(rdb).unwrap();
                                        Ok(())
                                    }
                                    Err(e) => {
                                        error!("Redb db is not created {:?}", e);
                                        return Err(Box::new(e));
                                    }
                                }
                            }
                            _ => {
                                error!("Redb db another io error {:?}", e);
                                return Err(Box::new(e));
                            }
                        }
                    }
                    _ => {
                        error!("Redb db another error {:?}", e);
                        return Err(Box::new(e));
                    }
                }
            }
        }
    }
    pub fn set(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            table.insert(key, &value)?;
        }
        write_txn.commit()?;
        Ok(())
    }
    fn to_db_key(protocol: &ProtocolName, network: &NetworkName) -> String {
        format!("{}-{}", protocol.to_string(), network.to_string())
    }
    pub fn get_blockchain(
        &self,
        protocol: &ProtocolName,
        network: &NetworkName,
    ) -> Result<blockchain::Blockchain, Box<dyn Error + Send + Sync>> {
        debug!("Redb get_blockchain({:?},{:?})", protocol, network);
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE)?;
        let key = Redb::to_db_key(protocol, network);
        let res = table.get(key.as_str())?;
        match res {
            Some(data) => {
                let blockchain: blockchain::Blockchain = serde_json::from_str(data.value())?;
                return Ok(blockchain);
            }
            None => {
                debug!("Redb get_blockchain return an empty response.");
                return Err("Error: Reddb return None".into());
            }
        }
    }

    pub fn set_blockchain(
        &self,
        blockchain: &blockchain::Blockchain,
        protocol: &ProtocolName,
        network: &NetworkName,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Redb set_blockchain({:?},{:?})", protocol, network);
        let chain_db = self.get_blockchain(&protocol, &network);
        let chain_db = match chain_db {
            Ok(data) => data,
            Err(e) => {
                error!(
                    "Redb get_blockchain return an error {:?} init empty blockchain",
                    e
                );
                blockchain::Blockchain::new(None)
            }
        };
        if chain_db.height == blockchain.height || chain_db.height > blockchain.height {
            debug!("Redb set_blockchain: same height, do nothing");
            return Ok(());
        }
        debug!(
            "Detect new chain height {} vs {}, updating database.",
            chain_db.height, blockchain.height
        );

        let keep = CONFIGURATION.get().unwrap().database.keep_history;

        let mut merged_blocks: Vec<Block> = chain_db
            .blocks
            .clone()
            .into_iter()
            .chain(blockchain.blocks.clone().into_iter())
            .fold(Vec::new(), |mut acc, b| {
                // if block is not in acc, add it
                if !acc.iter().any(|block: &Block| block.height == b.height) {
                    trace!("Add block {}", b.height);
                    acc.push(b);
                } else {
                    // by using chain on param blockchain at second arg, we ensure blockchain param have priority and will replace the block in db
                    if let Some(idx) = acc.iter().position(|block| block.height == b.height) {
                        trace!("Replace block {} with {}", acc[idx].height, b.height);
                        acc[idx] = b;
                    }
                }
                acc
            });
        merged_blocks.sort_by(|a, b| b.height.cmp(&a.height));
        let merged_blocks = merged_blocks.iter().take(keep as usize).cloned().collect();
        let mut blockchain = blockchain.clone();
        blockchain.blocks = merged_blocks;
        let key = Redb::to_db_key(&protocol, &network);
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE)?;
            let json_value = serde_json::to_string(&blockchain)?;
            table.insert(key.as_str(), json_value.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }
}
