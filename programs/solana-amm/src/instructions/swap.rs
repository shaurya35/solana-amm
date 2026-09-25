use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};

use crate::{
    constants::{BPS_DENOMINATOR, POOL_SEED},
    error::AmmError,
    instructions::add_liquidity::assert_reserves,
    math::{checked_mul_div_floor, swap_output},
    state::Pool,
};

pub fn handler(
    ctx: Context<Swap>,
    amount_in: u64,
    min_amount_out: u64,
    a_to_b: bool,
) -> Result<()> {
    require!(amount_in > 0, AmmError::ZeroAmount);
    assert_reserves(
        &ctx.accounts.pool,
        &ctx.accounts.vault_a,
        &ctx.accounts.vault_b,
    )?;

    let pool = &ctx.accounts.pool;
    let (
        expected_source_mint,
        expected_destination_mint,
        expected_treasury,
        reserve_in,
        reserve_out,
    ) = if a_to_b {
        (
            pool.mint_a,
            pool.mint_b,
            pool.treasury_a,
            pool.reserve_a,
            pool.reserve_b,
        )
    } else {
        (
            pool.mint_b,
            pool.mint_a,
            pool.treasury_b,
            pool.reserve_b,
            pool.reserve_a,
        )
    };

    require!(
        ctx.accounts.user_source.mint == expected_source_mint
            && ctx.accounts.user_source.owner == ctx.accounts.user.key()
            && ctx.accounts.user_destination.mint == expected_destination_mint
            && ctx.accounts.user_destination.owner == ctx.accounts.user.key()
            && ctx.accounts.treasury_input.key() == expected_treasury
            && ctx.accounts.treasury_input.mint == expected_source_mint
            && ctx.accounts.treasury_input.owner == pool.treasury_authority,
        AmmError::InvalidTokenAccount
    );

    let fee = checked_mul_div_floor(amount_in, pool.fee_bps as u64, BPS_DENOMINATOR)?;
    let net_input = amount_in.checked_sub(fee).ok_or(AmmError::MathOverflow)?;
    require!(net_input > 0, AmmError::FeeConsumesInput);
    let amount_out = swap_output(reserve_in, reserve_out, net_input)?;
    require!(amount_out > 0, AmmError::InsufficientLiquidity);
    require!(amount_out >= min_amount_out, AmmError::SlippageExceeded);

    let (vault_in, vault_out, mint_in, mint_out) = if a_to_b {
        (
            &ctx.accounts.vault_a,
            &ctx.accounts.vault_b,
            &ctx.accounts.mint_a,
            &ctx.accounts.mint_b,
        )
    } else {
        (
            &ctx.accounts.vault_b,
            &ctx.accounts.vault_a,
            &ctx.accounts.mint_b,
            &ctx.accounts.mint_a,
        )
    };

    if fee > 0 {
        transfer_from_user(
            &ctx.accounts.token_program,
            &ctx.accounts.user_source,
            &ctx.accounts.treasury_input,
            mint_in,
            &ctx.accounts.user,
            fee,
        )?;
    }
    transfer_from_user(
        &ctx.accounts.token_program,
        &ctx.accounts.user_source,
        vault_in,
        mint_in,
        &ctx.accounts.user,
        net_input,
    )?;

    let bump = [pool.bump];
    let signer_seeds: &[&[u8]] = &[POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref(), &bump];
    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: vault_out.to_account_info(),
                mint: mint_out.to_account_info(),
                to: ctx.accounts.user_destination.to_account_info(),
                authority: pool.to_account_info(),
            },
            &[signer_seeds],
        ),
        amount_out,
        mint_out.decimals,
    )?;

    let pool = &mut ctx.accounts.pool;
    if a_to_b {
        pool.reserve_a = pool
            .reserve_a
            .checked_add(net_input)
            .ok_or(AmmError::MathOverflow)?;
        pool.reserve_b = pool
            .reserve_b
            .checked_sub(amount_out)
            .ok_or(AmmError::MathOverflow)?;
    } else {
        pool.reserve_b = pool
            .reserve_b
            .checked_add(net_input)
            .ok_or(AmmError::MathOverflow)?;
        pool.reserve_a = pool
            .reserve_a
            .checked_sub(amount_out)
            .ok_or(AmmError::MathOverflow)?;
    }

    emit!(SwapExecuted {
        user: ctx.accounts.user.key(),
        a_to_b,
        amount_in,
        fee,
        amount_out,
    });
    Ok(())
}

fn transfer_from_user<'info>(
    token_program: &Program<'info, Token>,
    from: &Account<'info, TokenAccount>,
    to: &Account<'info, TokenAccount>,
    mint: &Account<'info, Mint>,
    user: &Signer<'info>,
    amount: u64,
) -> Result<()> {
    token::transfer_checked(
        CpiContext::new(
            token_program.key(),
            TransferChecked {
                from: from.to_account_info(),
                mint: mint.to_account_info(),
                to: to.to_account_info(),
                authority: user.to_account_info(),
            },
        ),
        amount,
        mint.decimals,
    )
}

#[derive(Accounts)]
pub struct Swap<'info> {
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,
    #[account(address = pool.mint_a)]
    pub mint_a: Box<Account<'info, Mint>>,
    #[account(address = pool.mint_b)]
    pub mint_b: Box<Account<'info, Mint>>,
    #[account(mut)]
    pub user_source: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_destination: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool.vault_a)]
    pub vault_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool.vault_b)]
    pub vault_b: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub treasury_input: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

#[event]
pub struct SwapExecuted {
    pub user: Pubkey,
    pub a_to_b: bool,
    pub amount_in: u64,
    pub fee: u64,
    pub amount_out: u64,
}
