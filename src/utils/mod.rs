use serde::{Deserialize, Deserializer};

//FIXME: Should be merge in same function with deserialize_from_hex_to_u128
pub fn deserialize_from_hex_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let hex_str = s.trim_start_matches("0x");
    let z = u64::from_str_radix(hex_str, 16);
    match z {
        Ok(z) => Ok(z),
        Err(e) => {
            error!("deserialize_from_hex_to_u64 error: {} {}", e, s);
            Err(serde::de::Error::custom(format!(
                "deserialize_from_hex_to_u64 error: {} {}",
                e, s,
            )))
        }
    }
}
// FIXME: Should be merge in same function with deserialize_from_hex_to_u64
pub fn deserialize_from_hex_to_u128<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let hex_str = s.trim_start_matches("0x");
    let z = u128::from_str_radix(hex_str, 16);
    match z {
        Ok(z) => Ok(z),
        Err(e) => {
            error!("deserialize_from_hex_to_u128 error: {} {}", e, s);
            Err(serde::de::Error::custom(format!(
                "deserialize_from_hex_to_u128 error:{} {}",
                e, s,
            )))
        }
    }
}
#[cfg(test)]
use crate::commons::blockchain;
#[cfg(test)]
pub fn assert_blockchain(b: blockchain::Blockchain) {
    // for each block in blockchain
    // next block height should be previous block height - i
    let previous_block_height = b.blocks[0].height;
    for (i, block) in b.blocks.iter().enumerate() {
        debug!(
            "block height: {} should be {}",
            block.height,
            previous_block_height - i as u64
        );
        assert_eq!(
            block.height,
            previous_block_height - i as u64,
            "block height: {} should be {}",
            block.height,
            previous_block_height - i as u64
        );
    }
}
