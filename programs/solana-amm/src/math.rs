use anchor_lang::prelude::*;

use crate::error::AmmError;

pub fn checked_mul_div_floor(a: u64, b: u64, denominator: u64) -> Result<u64> {
    require!(denominator > 0, AmmError::MathOverflow);
    let value = (a as u128)
        .checked_mul(b as u128)
        .ok_or(AmmError::MathOverflow)?
        .checked_div(denominator as u128)
        .ok_or(AmmError::MathOverflow)?;
    u64::try_from(value).map_err(|_| AmmError::MathOverflow.into())
}

pub fn checked_mul_div_ceil(a: u64, b: u64, denominator: u64) -> Result<u64> {
    require!(denominator > 0, AmmError::MathOverflow);
    let product = (a as u128)
        .checked_mul(b as u128)
        .ok_or(AmmError::MathOverflow)?;
    let value = product
        .checked_add((denominator - 1) as u128)
        .ok_or(AmmError::MathOverflow)?
        .checked_div(denominator as u128)
        .ok_or(AmmError::MathOverflow)?;
    u64::try_from(value).map_err(|_| AmmError::MathOverflow.into())
}

pub fn integer_sqrt(value: u128) -> u64 {
    if value < 2 {
        return value as u64;
    }

    let mut x = value;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + value / x) / 2;
    }

    x as u64
}

pub fn swap_output(reserve_in: u64, reserve_out: u64, net_input: u64) -> Result<u64> {
    require!(
        reserve_in > 0 && reserve_out > 0,
        AmmError::InsufficientLiquidity
    );
    let denominator = reserve_in
        .checked_add(net_input)
        .ok_or(AmmError::MathOverflow)?;
    checked_mul_div_floor(reserve_out, net_input, denominator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_root_is_integer_floor() {
        assert_eq!(integer_sqrt(0), 0);
        assert_eq!(integer_sqrt(1), 1);
        assert_eq!(integer_sqrt(15), 3);
        assert_eq!(integer_sqrt(16), 4);
        assert_eq!(integer_sqrt(10_000_000_000), 100_000);
    }

    #[test]
    fn cpmm_output_never_drains_reserve() {
        let out = swap_output(1_000_000, 2_000_000, 100_000).unwrap();
        assert_eq!(out, 181_818);
        assert!(out < 2_000_000);
    }

    #[test]
    fn ceil_and_floor_round_as_expected() {
        assert_eq!(checked_mul_div_floor(10, 2, 3).unwrap(), 6);
        assert_eq!(checked_mul_div_ceil(10, 2, 3).unwrap(), 7);
    }
}
