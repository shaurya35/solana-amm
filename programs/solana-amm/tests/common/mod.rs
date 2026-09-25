#![allow(dead_code)]

use anchor_lang::{prelude::Pubkey, AccountDeserialize, InstructionData, ToAccountMetas};
use litesvm::LiteSVM;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_program_pack::Pack;
use solana_signer::Signer;
use solana_system_interface::{instruction as system_instruction, program as system_program};
use solana_transaction::Transaction;
use spl_token_interface::{instruction as token_instruction, state::Account as SplAccount};

pub const DECIMALS: u8 = 6;
pub const INITIAL_A: u64 = 1_000_000;
pub const INITIAL_B: u64 = 2_000_000;
pub const INITIAL_SHARES: u64 = 1_414_213;

/// Boots a fresh in-process SVM with the built `solana_amm` program loaded.
pub fn new_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();
    svm.add_program(
        solana_amm::id(),
        include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/solana_amm.so"
        )),
    )
    .unwrap();
    svm
}

pub fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), String> {
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        signers,
        svm.latest_blockhash(),
    );
    let result = svm
        .send_transaction(transaction)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"));
    // Rotate the blockhash after every send so that back-to-back
    // transactions with identical instruction data (e.g. repeated swaps of
    // the same size) don't collide on signature and get rejected as
    // `AlreadyProcessed`.
    svm.expire_blockhash();
    result
}

pub fn airdrop(svm: &mut LiteSVM, key: &Pubkey) {
    svm.airdrop(key, 10_000_000_000).unwrap();
}

pub fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair) {
    let instructions = [
        system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            svm.minimum_balance_for_rent_exemption(spl_token_interface::state::Mint::LEN),
            spl_token_interface::state::Mint::LEN as u64,
            &spl_token_interface::ID,
        ),
        token_instruction::initialize_mint2(
            &spl_token_interface::ID,
            &mint.pubkey(),
            &payer.pubkey(),
            None,
            DECIMALS,
        )
        .unwrap(),
    ];
    send(svm, payer, &instructions, &[payer, mint]).unwrap();
}

pub fn create_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    account: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
) {
    let instructions = [
        system_instruction::create_account(
            &payer.pubkey(),
            &account.pubkey(),
            svm.minimum_balance_for_rent_exemption(SplAccount::LEN),
            SplAccount::LEN as u64,
            &spl_token_interface::ID,
        ),
        token_instruction::initialize_account3(
            &spl_token_interface::ID,
            &account.pubkey(),
            &mint,
            &owner,
        )
        .unwrap(),
    ];
    send(svm, payer, &instructions, &[payer, account]).unwrap();
}

pub fn mint_to(svm: &mut LiteSVM, payer: &Keypair, mint: Pubkey, destination: Pubkey, amount: u64) {
    let instruction = token_instruction::mint_to_checked(
        &spl_token_interface::ID,
        &mint,
        &destination,
        &payer.pubkey(),
        &[],
        amount,
        DECIMALS,
    )
    .unwrap();
    send(svm, payer, &[instruction], &[payer]).unwrap();
}

pub fn token_balance(svm: &LiteSVM, address: Pubkey) -> u64 {
    SplAccount::unpack(&svm.get_account(&address).unwrap().data)
        .unwrap()
        .amount
}

pub fn pool_state(svm: &LiteSVM, address: Pubkey) -> solana_amm::state::Pool {
    let account = svm.get_account(&address).unwrap();
    solana_amm::state::Pool::try_deserialize(&mut account.data.as_slice()).unwrap()
}

pub fn position_state(svm: &LiteSVM, address: Pubkey) -> solana_amm::state::Position {
    let account = svm.get_account(&address).unwrap();
    solana_amm::state::Position::try_deserialize(&mut account.data.as_slice()).unwrap()
}

/// The pool, vault, and treasury PDAs derived from an (ordered) mint pair.
pub struct PoolPdas {
    pub pool: Pubkey,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub treasury_a: Pubkey,
    pub treasury_b: Pubkey,
}

pub fn pool_pdas(mint_a: Pubkey, mint_b: Pubkey) -> PoolPdas {
    let program_id = solana_amm::id();
    let (pool, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::POOL_SEED,
            mint_a.as_ref(),
            mint_b.as_ref(),
        ],
        &program_id,
    );
    let (vault_a, _) = Pubkey::find_program_address(
        &[solana_amm::constants::VAULT_A_SEED, pool.as_ref()],
        &program_id,
    );
    let (vault_b, _) = Pubkey::find_program_address(
        &[solana_amm::constants::VAULT_B_SEED, pool.as_ref()],
        &program_id,
    );
    let (treasury_a, _) = Pubkey::find_program_address(
        &[solana_amm::constants::TREASURY_A_SEED, pool.as_ref()],
        &program_id,
    );
    let (treasury_b, _) = Pubkey::find_program_address(
        &[solana_amm::constants::TREASURY_B_SEED, pool.as_ref()],
        &program_id,
    );
    PoolPdas {
        pool,
        vault_a,
        vault_b,
        treasury_a,
        treasury_b,
    }
}

pub fn position_pda(pool: Pubkey, owner: Pubkey) -> Pubkey {
    let (position, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::POSITION_SEED,
            pool.as_ref(),
            owner.as_ref(),
        ],
        &solana_amm::id(),
    );
    position
}

#[allow(clippy::too_many_arguments)]
pub fn initialize_pool_ix(
    payer: Pubkey,
    authority: Pubkey,
    treasury_authority: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    fee_bps: u16,
) -> Instruction {
    let pdas = pool_pdas(mint_a, mint_b);
    Instruction::new_with_bytes(
        solana_amm::id(),
        &solana_amm::instruction::InitializePool { fee_bps }.data(),
        solana_amm::accounts::InitializePool {
            payer,
            authority,
            treasury_authority,
            mint_a,
            mint_b,
            pool: pdas.pool,
            vault_a: pdas.vault_a,
            vault_b: pdas.vault_b,
            treasury_a: pdas.treasury_a,
            treasury_b: pdas.treasury_b,
            token_program: spl_token_interface::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

pub fn open_position_ix(payer: Pubkey, owner: Pubkey, pool: Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        solana_amm::id(),
        &solana_amm::instruction::OpenPosition {}.data(),
        solana_amm::accounts::OpenPosition {
            payer,
            owner,
            pool,
            position: position_pda(pool, owner),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn add_liquidity_ix(
    owner: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    user_a: Pubkey,
    user_b: Pubkey,
    max_amount_a: u64,
    max_amount_b: u64,
    min_shares: u64,
) -> Instruction {
    let pdas = pool_pdas(mint_a, mint_b);
    Instruction::new_with_bytes(
        solana_amm::id(),
        &solana_amm::instruction::AddLiquidity {
            max_amount_a,
            max_amount_b,
            min_shares,
        }
        .data(),
        solana_amm::accounts::AddLiquidity {
            owner,
            pool: pdas.pool,
            position: position_pda(pdas.pool, owner),
            mint_a,
            mint_b,
            user_a,
            user_b,
            vault_a: pdas.vault_a,
            vault_b: pdas.vault_b,
            token_program: spl_token_interface::ID,
        }
        .to_account_metas(None),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn swap_ix(
    user: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    user_source: Pubkey,
    user_destination: Pubkey,
    amount_in: u64,
    min_amount_out: u64,
    a_to_b: bool,
) -> Instruction {
    let pdas = pool_pdas(mint_a, mint_b);
    let treasury_input = if a_to_b {
        pdas.treasury_a
    } else {
        pdas.treasury_b
    };
    Instruction::new_with_bytes(
        solana_amm::id(),
        &solana_amm::instruction::Swap {
            amount_in,
            min_amount_out,
            a_to_b,
        }
        .data(),
        solana_amm::accounts::Swap {
            user,
            pool: pdas.pool,
            mint_a,
            mint_b,
            user_source,
            user_destination,
            vault_a: pdas.vault_a,
            vault_b: pdas.vault_b,
            treasury_input,
            token_program: spl_token_interface::ID,
        }
        .to_account_metas(None),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn remove_liquidity_ix(
    owner: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    user_a: Pubkey,
    user_b: Pubkey,
    shares: u64,
    min_amount_a: u64,
    min_amount_b: u64,
) -> Instruction {
    let pdas = pool_pdas(mint_a, mint_b);
    Instruction::new_with_bytes(
        solana_amm::id(),
        &solana_amm::instruction::RemoveLiquidity {
            shares,
            min_amount_a,
            min_amount_b,
        }
        .data(),
        solana_amm::accounts::RemoveLiquidity {
            owner,
            pool: pdas.pool,
            position: position_pda(pdas.pool, owner),
            mint_a,
            mint_b,
            user_a,
            user_b,
            vault_a: pdas.vault_a,
            vault_b: pdas.vault_b,
            token_program: spl_token_interface::ID,
        }
        .to_account_metas(None),
    )
}

pub fn update_fee_ix(
    authority: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    new_fee_bps: u16,
) -> Instruction {
    let pdas = pool_pdas(mint_a, mint_b);
    Instruction::new_with_bytes(
        solana_amm::id(),
        &solana_amm::instruction::UpdateFee { new_fee_bps }.data(),
        solana_amm::accounts::UpdateFee {
            authority,
            pool: pdas.pool,
        }
        .to_account_metas(None),
    )
}

/// A pool that has been initialized, given a position for `payer`, and
/// seeded with `INITIAL_A` / `INITIAL_B` of liquidity, plus funded trader
/// token accounts ready to swap. Shared by most instruction-level tests so
/// each test body only has to set up the one behavior it is checking.
pub struct Fixture {
    pub payer: Keypair,
    pub treasury_authority: Keypair,
    pub trader: Keypair,
    pub attacker: Keypair,
    pub mint_a: Keypair,
    pub mint_b: Keypair,
    pub pdas: PoolPdas,
    pub position: Pubkey,
    pub provider_a: Keypair,
    pub provider_b: Keypair,
    pub trader_a: Keypair,
    pub trader_b: Keypair,
}

pub fn setup_seeded_pool(svm: &mut LiteSVM, fee_bps: u16) -> Fixture {
    let payer = Keypair::new();
    let treasury_authority = Keypair::new();
    let trader = Keypair::new();
    let attacker = Keypair::new();
    let mint_1 = Keypair::new();
    let mint_2 = Keypair::new();
    let (mint_a, mint_b) = if mint_1.pubkey().to_bytes() < mint_2.pubkey().to_bytes() {
        (mint_1, mint_2)
    } else {
        (mint_2, mint_1)
    };

    for key in [
        payer.pubkey(),
        treasury_authority.pubkey(),
        trader.pubkey(),
        attacker.pubkey(),
    ] {
        airdrop(svm, &key);
    }

    create_mint(svm, &payer, &mint_a);
    create_mint(svm, &payer, &mint_b);

    let provider_a = Keypair::new();
    let provider_b = Keypair::new();
    let trader_a = Keypair::new();
    let trader_b = Keypair::new();
    create_token_account(svm, &payer, &provider_a, mint_a.pubkey(), payer.pubkey());
    create_token_account(svm, &payer, &provider_b, mint_b.pubkey(), payer.pubkey());
    create_token_account(svm, &payer, &trader_a, mint_a.pubkey(), trader.pubkey());
    create_token_account(svm, &payer, &trader_b, mint_b.pubkey(), trader.pubkey());
    mint_to(svm, &payer, mint_a.pubkey(), provider_a.pubkey(), 5_000_000);
    mint_to(
        svm,
        &payer,
        mint_b.pubkey(),
        provider_b.pubkey(),
        10_000_000,
    );
    mint_to(svm, &payer, mint_a.pubkey(), trader_a.pubkey(), 1_000_000);
    mint_to(svm, &payer, mint_b.pubkey(), trader_b.pubkey(), 1_000_000);

    let pdas = pool_pdas(mint_a.pubkey(), mint_b.pubkey());
    let position = position_pda(pdas.pool, payer.pubkey());

    send(
        svm,
        &payer,
        &[initialize_pool_ix(
            payer.pubkey(),
            payer.pubkey(),
            treasury_authority.pubkey(),
            mint_a.pubkey(),
            mint_b.pubkey(),
            fee_bps,
        )],
        &[&payer],
    )
    .unwrap();

    send(
        svm,
        &payer,
        &[open_position_ix(payer.pubkey(), payer.pubkey(), pdas.pool)],
        &[&payer],
    )
    .unwrap();

    send(
        svm,
        &payer,
        &[add_liquidity_ix(
            payer.pubkey(),
            mint_a.pubkey(),
            mint_b.pubkey(),
            provider_a.pubkey(),
            provider_b.pubkey(),
            INITIAL_A,
            INITIAL_B,
            INITIAL_SHARES,
        )],
        &[&payer],
    )
    .unwrap();

    Fixture {
        payer,
        treasury_authority,
        trader,
        attacker,
        mint_a,
        mint_b,
        pdas,
        position,
        provider_a,
        provider_b,
        trader_a,
        trader_b,
    }
}
