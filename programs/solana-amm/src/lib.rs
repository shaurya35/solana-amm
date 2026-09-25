pub mod constants;
pub mod error;
pub mod instructions;
pub mod math;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("3SMKDteeV8JcQCxiq3vN3bJL2WvtHS1cGMXkdwqQDywD");

#[program]
pub mod solana_amm {
    use super::*;

    pub fn initialize_pool(ctx: Context<InitializePool>, fee_bps: u16) -> Result<()> {
        instructions::initialize_pool::handler(ctx, fee_bps)
    }

    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        max_amount_a: u64,
        max_amount_b: u64,
        min_shares: u64,
    ) -> Result<()> {
        instructions::add_liquidity::handler(ctx, max_amount_a, max_amount_b, min_shares)
    }

    pub fn open_position(ctx: Context<OpenPosition>) -> Result<()> {
        instructions::add_liquidity::handle_open_position(ctx)
    }

    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
        a_to_b: bool,
    ) -> Result<()> {
        instructions::swap::handler(ctx, amount_in, min_amount_out, a_to_b)
    }

    pub fn remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        shares: u64,
        min_amount_a: u64,
        min_amount_b: u64,
    ) -> Result<()> {
        instructions::remove_liquidity::handler(ctx, shares, min_amount_a, min_amount_b)
    }

    pub fn update_fee(ctx: Context<UpdateFee>, new_fee_bps: u16) -> Result<()> {
        instructions::update_fee::handler(ctx, new_fee_bps)
    }
}
