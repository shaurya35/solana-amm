use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("The two pool mints must be different")]
    IdenticalMints,
    #[msg("Mint accounts must be supplied in canonical address order")]
    InvalidMintOrder,
    #[msg("The protocol fee exceeds the maximum of 10%")]
    FeeTooHigh,
    #[msg("The amount must be greater than zero")]
    ZeroAmount,
    #[msg("The initial deposit is too small")]
    InitialLiquidityTooSmall,
    #[msg("The requested deposit amounts cannot mint any shares")]
    DepositTooSmall,
    #[msg("The requested withdrawal cannot return any tokens")]
    WithdrawalTooSmall,
    #[msg("The swap output is below the user's minimum")]
    SlippageExceeded,
    #[msg("The liquidity output is below the user's minimum")]
    MinimumSharesNotMet,
    #[msg("The withdrawal output is below the user's minimum")]
    MinimumTokensNotMet,
    #[msg("The position does not contain enough shares")]
    InsufficientShares,
    #[msg("The pool has no usable liquidity")]
    InsufficientLiquidity,
    #[msg("The fee consumes the full swap input")]
    FeeConsumesInput,
    #[msg("A supplied token account has the wrong mint or authority")]
    InvalidTokenAccount,
    #[msg("Pool vault balances do not match the recorded reserves")]
    ReserveMismatch,
    #[msg("Only the pool authority may perform this action")]
    Unauthorized,
    #[msg("Arithmetic overflow or underflow")]
    MathOverflow,
}
