use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

pub const CONTRACT_NAME: &str = "crates.io:cl8y-faucet";
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const COOLDOWN_SECONDS: u64 = 86_400; // 24 hours
pub const CLAIM_AMOUNT: u128 = 10;

pub const ADMIN: Item<Addr> = Item::new("admin");

/// token_address => decimals
pub const TOKENS: Map<&str, u8> = Map::new("tokens");

/// (user_address, token_address) => last claim timestamp (seconds)
pub const LAST_CLAIM: Map<(&Addr, &str), u64> = Map::new("last_claim");
