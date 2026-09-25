use anchor_lang::prelude::*;

#[constant]
pub const POOL_SEED: &[u8] = b"pool";

#[constant]
pub const VAULT_A_SEED: &[u8] = b"vault-a";

#[constant]
pub const VAULT_B_SEED: &[u8] = b"vault-b";

#[constant]
pub const TREASURY_A_SEED: &[u8] = b"treasury-a";

#[constant]
pub const TREASURY_B_SEED: &[u8] = b"treasury-b";

#[constant]
pub const POSITION_SEED: &[u8] = b"position";

pub const BPS_DENOMINATOR: u64 = 10_000;
pub const MAX_FEE_BPS: u16 = 1_000;
pub const MINIMUM_LIQUIDITY: u64 = 1_000;
