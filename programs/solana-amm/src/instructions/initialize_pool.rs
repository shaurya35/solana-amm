use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    constants::{
        MAX_FEE_BPS, POOL_SEED, TREASURY_A_SEED, TREASURY_B_SEED, VAULT_A_SEED, VAULT_B_SEED,
    },
    error::AmmError,
    state::Pool,
};

pub fn handler(ctx: Context<InitializePool>, fee_bps: u16) -> Result<()> {
    require!(
        ctx.accounts.mint_a.key() != ctx.accounts.mint_b.key(),
        AmmError::IdenticalMints
    );
    require!(
        ctx.accounts.mint_a.key().to_bytes() < ctx.accounts.mint_b.key().to_bytes(),
        AmmError::InvalidMintOrder
    );
    require!(fee_bps <= MAX_FEE_BPS, AmmError::FeeTooHigh);

    let pool = &mut ctx.accounts.pool;
    pool.authority = ctx.accounts.authority.key();
    pool.treasury_authority = ctx.accounts.treasury_authority.key();
    pool.mint_a = ctx.accounts.mint_a.key();
    pool.mint_b = ctx.accounts.mint_b.key();
    pool.vault_a = ctx.accounts.vault_a.key();
    pool.vault_b = ctx.accounts.vault_b.key();
    pool.treasury_a = ctx.accounts.treasury_a.key();
    pool.treasury_b = ctx.accounts.treasury_b.key();
    pool.reserve_a = 0;
    pool.reserve_b = 0;
    pool.total_shares = 0;
    pool.fee_bps = fee_bps;
    pool.bump = ctx.bumps.pool;
    pool.vault_a_bump = ctx.bumps.vault_a;
    pool.vault_b_bump = ctx.bumps.vault_b;
    Ok(())
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    pub treasury_authority: SystemAccount<'info>,
    pub mint_a: Box<Account<'info, Mint>>,
    pub mint_b: Box<Account<'info, Mint>>,
    #[account(
        init,
        payer = payer,
        space = 8 + Pool::INIT_SPACE,
        seeds = [POOL_SEED, mint_a.key().as_ref(), mint_b.key().as_ref()],
        bump
    )]
    pub pool: Box<Account<'info, Pool>>,
    #[account(
        init,
        payer = payer,
        token::mint = mint_a,
        token::authority = pool,
        seeds = [VAULT_A_SEED, pool.key().as_ref()],
        bump
    )]
    pub vault_a: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = payer,
        token::mint = mint_b,
        token::authority = pool,
        seeds = [VAULT_B_SEED, pool.key().as_ref()],
        bump
    )]
    pub vault_b: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = payer,
        token::mint = mint_a,
        token::authority = treasury_authority,
        seeds = [TREASURY_A_SEED, pool.key().as_ref()],
        bump
    )]
    pub treasury_a: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = payer,
        token::mint = mint_b,
        token::authority = treasury_authority,
        seeds = [TREASURY_B_SEED, pool.key().as_ref()],
        bump
    )]
    pub treasury_b: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
