use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{
    constants::{MINIMUM_LIQUIDITY, POOL_SEED, POSITION_SEED, VAULT_A_SEED, VAULT_B_SEED},
    error::AmmError,
    math::{checked_mul_div_ceil, checked_mul_div_floor, integer_sqrt},
    state::{Pool, Position},
};

pub fn handle_open_position(ctx: Context<OpenPosition>) -> Result<()> {
    let position = &mut ctx.accounts.position;
    position.pool = ctx.accounts.pool.key();
    position.owner = ctx.accounts.owner.key();
    position.shares = 0;
    position.bump = ctx.bumps.position;
    Ok(())
}

pub fn handler(
    ctx: Context<AddLiquidity>,
    max_amount_a: u64,
    max_amount_b: u64,
    min_shares: u64,
) -> Result<()> {
    require!(max_amount_a > 0 && max_amount_b > 0, AmmError::ZeroAmount);
    assert_reserves(
        &ctx.accounts.pool,
        &ctx.accounts.vault_a,
        &ctx.accounts.vault_b,
    )?;

    let pool = &ctx.accounts.pool;
    let (amount_a, amount_b, shares) = if pool.total_shares == 0 {
        let product = (max_amount_a as u128)
            .checked_mul(max_amount_b as u128)
            .ok_or(AmmError::MathOverflow)?;
        let shares = integer_sqrt(product);
        require!(
            shares >= MINIMUM_LIQUIDITY,
            AmmError::InitialLiquidityTooSmall
        );
        (max_amount_a, max_amount_b, shares)
    } else {
        require!(
            pool.reserve_a > 0 && pool.reserve_b > 0,
            AmmError::ReserveMismatch
        );
        let shares_a = checked_mul_div_floor(max_amount_a, pool.total_shares, pool.reserve_a)?;
        let shares_b = checked_mul_div_floor(max_amount_b, pool.total_shares, pool.reserve_b)?;
        let shares = shares_a.min(shares_b);
        require!(shares > 0, AmmError::DepositTooSmall);
        let amount_a = checked_mul_div_ceil(shares, pool.reserve_a, pool.total_shares)?;
        let amount_b = checked_mul_div_ceil(shares, pool.reserve_b, pool.total_shares)?;
        require!(
            amount_a <= max_amount_a && amount_b <= max_amount_b,
            AmmError::MathOverflow
        );
        (amount_a, amount_b, shares)
    };

    require!(shares >= min_shares, AmmError::MinimumSharesNotMet);

    transfer_to_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.user_a,
        &ctx.accounts.vault_a,
        &ctx.accounts.mint_a,
        &ctx.accounts.owner,
        amount_a,
    )?;
    transfer_to_vault(
        &ctx.accounts.token_program,
        &ctx.accounts.user_b,
        &ctx.accounts.vault_b,
        &ctx.accounts.mint_b,
        &ctx.accounts.owner,
        amount_b,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.reserve_a = pool
        .reserve_a
        .checked_add(amount_a)
        .ok_or(AmmError::MathOverflow)?;
    pool.reserve_b = pool
        .reserve_b
        .checked_add(amount_b)
        .ok_or(AmmError::MathOverflow)?;
    pool.total_shares = pool
        .total_shares
        .checked_add(shares)
        .ok_or(AmmError::MathOverflow)?;
    ctx.accounts.position.shares = ctx
        .accounts
        .position
        .shares
        .checked_add(shares)
        .ok_or(AmmError::MathOverflow)?;

    emit!(LiquidityAdded {
        provider: ctx.accounts.owner.key(),
        amount_a,
        amount_b,
        shares,
    });
    Ok(())
}

fn transfer_to_vault<'info>(
    token_program: &Program<'info, Token>,
    from: &Account<'info, TokenAccount>,
    to: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    owner: &Signer<'info>,
    amount: u64,
) -> Result<()> {
    token::transfer_checked(
        CpiContext::new(
            token_program.key(),
            TransferChecked {
                from: from.to_account_info(),
                mint: mint.to_account_info(),
                to: to.to_account_info(),
                authority: owner.to_account_info(),
            },
        ),
        amount,
        mint.decimals,
    )
}

pub(crate) fn assert_reserves(
    pool: &Account<Pool>,
    vault_a: &Account<TokenAccount>,
    vault_b: &Account<TokenAccount>,
) -> Result<()> {
    require!(
        vault_a.amount == pool.reserve_a && vault_b.amount == pool.reserve_b,
        AmmError::ReserveMismatch
    );
    Ok(())
}

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub owner: Signer<'info>,
    #[account(
        seeds = [POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,
    #[account(
        init,
        payer = payer,
        space = 8 + Position::INIT_SPACE,
        seeds = [POSITION_SEED, pool.key().as_ref(), owner.key().as_ref()],
        bump
    )]
    pub position: Box<Account<'info, Position>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
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
    #[account(
        mut,
        address = pool.vault_a,
        seeds = [VAULT_A_SEED, pool.key().as_ref()],
        bump = pool.vault_a_bump
    )]
    pub vault_a: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        address = pool.vault_b,
        seeds = [VAULT_B_SEED, pool.key().as_ref()],
        bump = pool.vault_b_bump
    )]
    pub vault_b: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

#[event]
pub struct LiquidityAdded {
    pub provider: Pubkey,
    pub amount_a: u64,
    pub amount_b: u64,
    pub shares: u64,
}
