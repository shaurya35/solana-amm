# Solana AMM

Week 3 assignment: an AMM program with fees and a treasury. Plus both optional extras: the swap math is hand-written (no AMM library), and a section below on handling downtime.

Tests run locally with LiteSVM.

## What it does

- `initialize_pool`: sets up a pool for a pair of tokens, with vaults, a treasury, and a fee.
- `add_liquidity` / `open_position`: deposit both tokens, get LP shares back.
- `swap`: trade one token for the other using the constant-product formula (`x * y = k`), with slippage protection.
- `remove_liquidity`: burn shares, get your share of both tokens back.
- `update_fee`: only the pool's authority can change the fee.

Every swap takes a fee (in basis points, capped at 10%) and sends it to a treasury account, kept separate from the pool's own funds.

## No library

The math (`checked_mul_div`, integer square root, the swap formula) is written from scratch in `src/math.rs`. The program only depends on `anchor-lang` and `anchor-spl`, no AMM/DEX crate.

## Handling downtime

The program itself is never "down": once deployed, it just runs. The real risks live elsewhere:

- **A single RPC going down.** Fix by using more than one RPC endpoint from the client.
- **A frontend showing stale prices.** Not dangerous here, because every swap checks `min_amount_out` on-chain against the *current* reserves. A stale quote just fails instead of executing at a bad price.
- **Wanting an emergency stop.** The program doesn't have a pause switch yet. Adding an authority-gated `paused` flag, checked in `swap`/`add_liquidity`/`remove_liquidity`, would be the natural next step for production.

## Run it

```bash
NO_DNA=1 anchor build
cargo test --workspace -- --nocapture
```

![Tests passing](docs/test-results.png)

## Code

```
programs/solana-amm/src/
├── math.rs                        # swap math, no external library
├── state.rs                       # Pool and Position accounts
└── instructions/
    ├── initialize_pool.rs
    ├── add_liquidity.rs
    ├── swap.rs
    ├── remove_liquidity.rs
    └── update_fee.rs
```

10 tests across `tests/amm.rs` and `src/math.rs`, covering every instruction and its failure cases.

## License

MIT
