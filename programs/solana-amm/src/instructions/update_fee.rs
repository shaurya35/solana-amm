use anchor_lang::prelude::*;

use crate::{
    constants::{MAX_FEE_BPS, POOL_SEED},
    error::AmmError,
    state::Pool,
};

pub fn handler(ctx: Context<UpdateFee>, new_fee_bps: u16) -> Result<()> {
    require!(new_fee_bps <= MAX_FEE_BPS, AmmError::FeeTooHigh);
    let old_fee_bps = ctx.accounts.pool.fee_bps;
    ctx.accounts.pool.fee_bps = new_fee_bps;
    emit!(FeeUpdated {
        authority: ctx.accounts.authority.key(),
        old_fee_bps,
        new_fee_bps,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateFee<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority @ AmmError::Unauthorized,
        seeds = [POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,
}

#[event]
pub struct FeeUpdated {
    pub authority: Pubkey,
    pub old_fee_bps: u16,
    pub new_fee_bps: u16,
}
