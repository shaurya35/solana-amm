use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{
    constants::{POOL_SEED, POSITION_SEED},
    error::AmmError,
    instructions::add_liquidity::assert_reserves,
    math::checked_mul_div_floor,
    state::{Pool, Position},
};

pub fn handler(
    ctx: Context<RemoveLiquidity>,
    shares: u64,
    min_amount_a: u64,
    min_amount_b: u64,
) -> Result<()> {
    require!(shares > 0, AmmError::ZeroAmount);
    require!(
        ctx.accounts.position.shares >= shares,
        AmmError::InsufficientShares
    );
    assert_reserves(
        &ctx.accounts.pool,
        &ctx.accounts.vault_a,
        &ctx.accounts.vault_b,
    )?;

    let pool = &ctx.accounts.pool;
    require!(pool.total_shares > 0, AmmError::InsufficientLiquidity);
    let amount_a = checked_mul_div_floor(shares, pool.reserve_a, pool.total_shares)?;
    let amount_b = checked_mul_div_floor(shares, pool.reserve_b, pool.total_shares)?;
    require!(amount_a > 0 && amount_b > 0, AmmError::WithdrawalTooSmall);
    require!(
        amount_a >= min_amount_a && amount_b >= min_amount_b,
        AmmError::MinimumTokensNotMet
    );

    let bump = [pool.bump];
    let signer_seeds: &[&[u8]] = &[POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref(), &bump];
    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.vault_a,
        &ctx.accounts.user_a,
        &ctx.accounts.mint_a,
        pool,
        signer_seeds,
        amount_a,
    )?;
    transfer_from_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.vault_b,
        &ctx.accounts.user_b,
        &ctx.accounts.mint_b,
        pool,
        signer_seeds,
        amount_b,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.reserve_a = pool
        .reserve_a
        .checked_sub(amount_a)
        .ok_or(AmmError::MathOverflow)?;
    pool.reserve_b = pool
        .reserve_b
        .checked_sub(amount_b)
        .ok_or(AmmError::MathOverflow)?;
    pool.total_shares = pool
        .total_shares
        .checked_sub(shares)
        .ok_or(AmmError::MathOverflow)?;
    ctx.accounts.position.shares = ctx
        .accounts
        .position
        .shares
        .checked_sub(shares)
        .ok_or(AmmError::MathOverflow)?;

    emit!(LiquidityRemoved {
        provider: ctx.accounts.owner.key(),
        amount_a,
        amount_b,
        shares,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn transfer_from_vault<'info>(
    token_program: &Program<'info, Token>,
    from: &Account<'info, TokenAccount>,
    to: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    pool: &Account<'info, Pool>,
    signer_seeds: &[&[u8]],
    amount: u64,
) -> Result<()> {
    token::transfer_checked(
        CpiContext::new_with_signer(
            token_program.key(),
            TransferChecked {
                from: from.to_account_info(),
                mint: mint.to_account_info(),
                to: to.to_account_info(),
                authority: pool.to_account_info(),
            },
            &[signer_seeds],
        ),
        amount,
        mint.decimals,
    )
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    pub owner: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,
    #[account(
        mut,
        has_one = pool,
        has_one = owner,
        seeds = [POSITION_SEED, pool.key().as_ref(), owner.key().as_ref()],
        bump = position.bump
    )]
    pub position: Box<Account<'info, Position>>,
    #[account(address = pool.mint_a)]
    pub mint_a: Box<Account<'info, Mint>>,
    #[account(address = pool.mint_b)]
    pub mint_b: Box<Account<'info, Mint>>,
    #[account(mut, token::mint = mint_a, token::authority = owner)]
    pub user_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = mint_b, token::authority = owner)]
    pub user_b: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool.vault_a)]
    pub vault_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool.vault_b)]
    pub vault_b: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

#[event]
pub struct LiquidityRemoved {
    pub provider: Pubkey,
    pub amount_a: u64,
    pub amount_b: u64,
    pub shares: u64,
}
