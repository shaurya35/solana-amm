mod common;

use solana_keypair::Keypair;
use solana_signer::Signer;

use common::*;

/// Exercises the full instruction set end to end: initialize the pool,
/// open a position, seed liquidity, swap with a fee routed to the
/// treasury, gate the fee update behind the pool authority, and withdraw
/// liquidity. This is the "happy path plus the headline security checks"
/// test; narrower failure modes get their own focused tests below.
#[test]
fn complete_cpmm_lifecycle_with_fees_and_security_checks() {
    let mut svm = new_svm();
    let fx = setup_seeded_pool(&mut svm, 30);

    let initialized = pool_state(&svm, fx.pdas.pool);
    assert_eq!(initialized.reserve_a, INITIAL_A);
    assert_eq!(initialized.reserve_b, INITIAL_B);
    assert_eq!(initialized.total_shares, INITIAL_SHARES);
    assert_eq!(position_state(&svm, fx.position).shares, INITIAL_SHARES);
    assert_eq!(token_balance(&svm, fx.pdas.vault_a), INITIAL_A);
    assert_eq!(token_balance(&svm, fx.pdas.vault_b), INITIAL_B);

    let swap = |minimum_output: u64| {
        swap_ix(
            fx.trader.pubkey(),
            fx.mint_a.pubkey(),
            fx.mint_b.pubkey(),
            fx.trader_a.pubkey(),
            fx.trader_b.pubkey(),
            100_000,
            minimum_output,
            true,
        )
    };

    let rejected = send(&mut svm, &fx.trader, &[swap(190_000)], &[&fx.trader]);
    assert!(rejected.is_err(), "an impossible minimum output must fail");
    assert_eq!(token_balance(&svm, fx.pdas.treasury_a), 0);
    assert_eq!(token_balance(&svm, fx.trader_a.pubkey()), 1_000_000);

    send(&mut svm, &fx.trader, &[swap(181_000)], &[&fx.trader]).unwrap();
    let after_swap = pool_state(&svm, fx.pdas.pool);
    assert_eq!(token_balance(&svm, fx.pdas.treasury_a), 300);
    assert_eq!(token_balance(&svm, fx.pdas.treasury_b), 0);
    assert_eq!(after_swap.reserve_a, 1_099_700);
    assert_eq!(after_swap.reserve_b, 1_818_678);
    assert_eq!(token_balance(&svm, fx.trader_a.pubkey()), 900_000);
    assert_eq!(token_balance(&svm, fx.trader_b.pubkey()), 1_181_322);
    assert!(
        (after_swap.reserve_a as u128) * (after_swap.reserve_b as u128)
            >= (INITIAL_A as u128) * (INITIAL_B as u128),
        "the constant-product invariant must never decrease from a swap"
    );

    let unauthorized_update = update_fee_ix(
        fx.attacker.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        100,
    );
    assert!(
        send(
            &mut svm,
            &fx.attacker,
            &[unauthorized_update],
            &[&fx.attacker]
        )
        .is_err(),
        "only the pool authority may update the fee"
    );
    assert_eq!(pool_state(&svm, fx.pdas.pool).fee_bps, 30);

    let authorized_update = update_fee_ix(
        fx.payer.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        100,
    );
    send(&mut svm, &fx.payer, &[authorized_update], &[&fx.payer]).unwrap();
    assert_eq!(pool_state(&svm, fx.pdas.pool).fee_bps, 100);

    let before_withdrawal = pool_state(&svm, fx.pdas.pool);
    let shares_to_remove = INITIAL_SHARES / 2;
    let expected_a = ((shares_to_remove as u128 * before_withdrawal.reserve_a as u128)
        / before_withdrawal.total_shares as u128) as u64;
    let expected_b = ((shares_to_remove as u128 * before_withdrawal.reserve_b as u128)
        / before_withdrawal.total_shares as u128) as u64;
    let remove = remove_liquidity_ix(
        fx.payer.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        fx.provider_a.pubkey(),
        fx.provider_b.pubkey(),
        shares_to_remove,
        expected_a,
        expected_b,
    );
    send(&mut svm, &fx.payer, &[remove], &[&fx.payer]).unwrap();

    let after_withdrawal = pool_state(&svm, fx.pdas.pool);
    assert_eq!(
        after_withdrawal.total_shares,
        INITIAL_SHARES - shares_to_remove
    );
    assert_eq!(
        position_state(&svm, fx.position).shares,
        INITIAL_SHARES - shares_to_remove
    );
    assert_eq!(
        after_withdrawal.reserve_a,
        before_withdrawal.reserve_a - expected_a
    );
    assert_eq!(
        after_withdrawal.reserve_b,
        before_withdrawal.reserve_b - expected_b
    );
    assert_eq!(
        token_balance(&svm, fx.pdas.vault_a),
        after_withdrawal.reserve_a
    );
    assert_eq!(
        token_balance(&svm, fx.pdas.vault_b),
        after_withdrawal.reserve_b
    );
}

/// `initialize_pool` must reject a mint pair that is identical or supplied
/// out of canonical order, since the pool PDA and swap-direction logic both
/// assume `mint_a < mint_b`.
#[test]
fn initialize_pool_validates_mint_identity_and_order() {
    let mut svm = new_svm();
    let payer = Keypair::new();
    let treasury_authority = Keypair::new();
    airdrop(&mut svm, &payer.pubkey());

    let mint_1 = Keypair::new();
    let mint_2 = Keypair::new();
    let (mint_a, mint_b) = if mint_1.pubkey().to_bytes() < mint_2.pubkey().to_bytes() {
        (mint_1, mint_2)
    } else {
        (mint_2, mint_1)
    };
    create_mint(&mut svm, &payer, &mint_a);
    create_mint(&mut svm, &payer, &mint_b);

    let identical = initialize_pool_ix(
        payer.pubkey(),
        payer.pubkey(),
        treasury_authority.pubkey(),
        mint_a.pubkey(),
        mint_a.pubkey(),
        30,
    );
    assert!(
        send(&mut svm, &payer, &[identical], &[&payer]).is_err(),
        "identical mints must be rejected"
    );

    let out_of_order = initialize_pool_ix(
        payer.pubkey(),
        payer.pubkey(),
        treasury_authority.pubkey(),
        mint_b.pubkey(),
        mint_a.pubkey(),
        30,
    );
    assert!(
        send(&mut svm, &payer, &[out_of_order], &[&payer]).is_err(),
        "mints supplied out of canonical order must be rejected"
    );
}

/// `fee_bps` is capped at `MAX_FEE_BPS` (10%) both when a pool is created
/// and whenever the authority later updates it.
#[test]
fn fee_bps_is_capped_at_initialization_and_at_update() {
    let mut svm = new_svm();
    let payer = Keypair::new();
    let treasury_authority = Keypair::new();
    airdrop(&mut svm, &payer.pubkey());

    let mint_1 = Keypair::new();
    let mint_2 = Keypair::new();
    let (mint_a, mint_b) = if mint_1.pubkey().to_bytes() < mint_2.pubkey().to_bytes() {
        (mint_1, mint_2)
    } else {
        (mint_2, mint_1)
    };
    create_mint(&mut svm, &payer, &mint_a);
    create_mint(&mut svm, &payer, &mint_b);

    let over_cap_init = initialize_pool_ix(
        payer.pubkey(),
        payer.pubkey(),
        treasury_authority.pubkey(),
        mint_a.pubkey(),
        mint_b.pubkey(),
        1_001,
    );
    assert!(
        send(&mut svm, &payer, &[over_cap_init], &[&payer]).is_err(),
        "initialize_pool must reject a fee above MAX_FEE_BPS"
    );

    let ok_init = initialize_pool_ix(
        payer.pubkey(),
        payer.pubkey(),
        treasury_authority.pubkey(),
        mint_a.pubkey(),
        mint_b.pubkey(),
        1_000,
    );
    send(&mut svm, &payer, &[ok_init], &[&payer]).unwrap();

    let over_cap_update = update_fee_ix(payer.pubkey(), mint_a.pubkey(), mint_b.pubkey(), 1_001);
    assert!(
        send(&mut svm, &payer, &[over_cap_update], &[&payer]).is_err(),
        "update_fee must reject a fee above MAX_FEE_BPS"
    );
    let pdas = pool_pdas(mint_a.pubkey(), mint_b.pubkey());
    assert_eq!(pool_state(&svm, pdas.pool).fee_bps, 1_000);
}

/// Every instruction that takes a caller-supplied amount rejects zero,
/// instead of silently succeeding as a no-op that would still charge rent
/// or emit misleading events.
#[test]
fn zero_amount_inputs_are_rejected_across_instructions() {
    let mut svm = new_svm();
    let fx = setup_seeded_pool(&mut svm, 30);

    let zero_add = add_liquidity_ix(
        fx.payer.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        fx.provider_a.pubkey(),
        fx.provider_b.pubkey(),
        0,
        0,
        0,
    );
    assert!(
        send(&mut svm, &fx.payer, &[zero_add], &[&fx.payer]).is_err(),
        "add_liquidity must reject a zero-amount deposit"
    );

    let zero_swap = swap_ix(
        fx.trader.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        fx.trader_a.pubkey(),
        fx.trader_b.pubkey(),
        0,
        0,
        true,
    );
    assert!(
        send(&mut svm, &fx.trader, &[zero_swap], &[&fx.trader]).is_err(),
        "swap must reject a zero amount_in"
    );

    let zero_remove = remove_liquidity_ix(
        fx.payer.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        fx.provider_a.pubkey(),
        fx.provider_b.pubkey(),
        0,
        0,
        0,
    );
    assert!(
        send(&mut svm, &fx.payer, &[zero_remove], &[&fx.payer]).is_err(),
        "remove_liquidity must reject a zero-share withdrawal"
    );

    // Reserves and total_shares are unchanged: every rejected call rolled back.
    let pool = pool_state(&svm, fx.pdas.pool);
    assert_eq!(pool.reserve_a, INITIAL_A);
    assert_eq!(pool.reserve_b, INITIAL_B);
    assert_eq!(pool.total_shares, INITIAL_SHARES);
}

/// A `b_to_a` swap must debit the fee into `treasury_b` (not `treasury_a`),
/// credit the trader in mint A, and keep the constant-product invariant
/// intact, mirroring the `a_to_b` path exercised by the lifecycle test.
#[test]
fn swap_b_to_a_direction_credits_treasury_b_and_preserves_invariant() {
    let mut svm = new_svm();
    let fx = setup_seeded_pool(&mut svm, 50);

    let before = pool_state(&svm, fx.pdas.pool);
    let swap = swap_ix(
        fx.trader.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        fx.trader_b.pubkey(),
        fx.trader_a.pubkey(),
        200_000,
        1,
        false,
    );
    send(&mut svm, &fx.trader, &[swap], &[&fx.trader]).unwrap();

    let after = pool_state(&svm, fx.pdas.pool);
    assert!(
        token_balance(&svm, fx.pdas.treasury_b) > 0,
        "the fee must land in treasury_b"
    );
    assert_eq!(token_balance(&svm, fx.pdas.treasury_a), 0);
    assert!(after.reserve_b > before.reserve_b);
    assert!(after.reserve_a < before.reserve_a);
    assert!(
        (after.reserve_a as u128) * (after.reserve_b as u128)
            >= (before.reserve_a as u128) * (before.reserve_b as u128)
    );
}

/// Running several swaps back to back must monotonically grow the
/// treasury balance: fees never regress and never leak anywhere else.
#[test]
fn treasury_balance_grows_monotonically_across_repeated_swaps() {
    let mut svm = new_svm();
    let fx = setup_seeded_pool(&mut svm, 25);

    let mut previous_treasury_balance = 0u64;
    for _ in 0..3 {
        let swap = swap_ix(
            fx.trader.pubkey(),
            fx.mint_a.pubkey(),
            fx.mint_b.pubkey(),
            fx.trader_a.pubkey(),
            fx.trader_b.pubkey(),
            50_000,
            1,
            true,
        );
        send(&mut svm, &fx.trader, &[swap], &[&fx.trader]).unwrap();
        let current = token_balance(&svm, fx.pdas.treasury_a);
        assert!(
            current > previous_treasury_balance,
            "treasury_a balance must strictly increase after each fee-bearing swap"
        );
        previous_treasury_balance = current;
    }
}

/// `add_liquidity` and `remove_liquidity` both honor their caller-supplied
/// slippage floors (`min_shares`, `min_amount_a`/`min_amount_b`), and
/// `remove_liquidity` refuses to burn more shares than a position holds.
#[test]
fn liquidity_slippage_and_share_limits_are_enforced() {
    let mut svm = new_svm();
    let fx = setup_seeded_pool(&mut svm, 30);

    // A deposit sized to mint far fewer shares than the caller demands.
    let unreachable_min_shares = add_liquidity_ix(
        fx.payer.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        fx.provider_a.pubkey(),
        fx.provider_b.pubkey(),
        1_000,
        2_000,
        1_000_000,
    );
    assert!(
        send(&mut svm, &fx.payer, &[unreachable_min_shares], &[&fx.payer]).is_err(),
        "add_liquidity must reject a deposit that cannot mint min_shares"
    );

    // A withdrawal that demands more tokens out than the share count entitles.
    let unreachable_min_amounts = remove_liquidity_ix(
        fx.payer.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        fx.provider_a.pubkey(),
        fx.provider_b.pubkey(),
        1_000,
        1_000_000,
        1_000_000,
    );
    assert!(
        send(
            &mut svm,
            &fx.payer,
            &[unreachable_min_amounts],
            &[&fx.payer]
        )
        .is_err(),
        "remove_liquidity must reject a withdrawal below the caller's minimums"
    );

    // Attempting to burn more shares than the position holds.
    let too_many_shares = remove_liquidity_ix(
        fx.payer.pubkey(),
        fx.mint_a.pubkey(),
        fx.mint_b.pubkey(),
        fx.provider_a.pubkey(),
        fx.provider_b.pubkey(),
        INITIAL_SHARES + 1,
        0,
        0,
    );
    assert!(
        send(&mut svm, &fx.payer, &[too_many_shares], &[&fx.payer]).is_err(),
        "remove_liquidity must reject burning more shares than the position holds"
    );

    // Pool state is untouched: every rejected call rolled back.
    let pool = pool_state(&svm, fx.pdas.pool);
    assert_eq!(pool.total_shares, INITIAL_SHARES);
    assert_eq!(position_state(&svm, fx.position).shares, INITIAL_SHARES);
}
