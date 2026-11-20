use crate::error::AmmError;
use anchor_lang::prelude::*;

const BPS: u128 = 10_000;

pub fn proportional(reserve: u64, shares: u64, supply: u64, round_up: bool) -> Result<u64> {
    require!(supply > 0, AmmError::NoLiquidityInPool);
    let n = u128::from(reserve) * u128::from(shares);
    let d = u128::from(supply);
    let q = n / d + if round_up && n % d != 0 { 1 } else { 0 };
    u64::try_from(q).map_err(|_| error!(AmmError::Overflow))
}

// Split the rounded input fee: 20% treasury, 80% liquidity providers.
pub fn quote(x: u64, y: u64, amount: u64, fee_bps: u16) -> Result<(u64, u64)> {
    require!(x > 0 && y > 0, AmmError::NoLiquidityInPool);
    require!(amount > 0, AmmError::InvalidAmount);
    require!(u128::from(fee_bps) < BPS, AmmError::InvalidFee);
    let fee = u128::from(amount) * u128::from(fee_bps) / BPS;
    let net = u128::from(amount) - fee;
    let out = u128::from(y) * net / (u128::from(x) + net);
    require!(out > 0 && out < u128::from(y), AmmError::InvalidAmount);
    Ok((out as u64, (fee / 5) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proportional_rounding() {
        assert_eq!(proportional(10, 1, 3, true).unwrap(), 4);
        assert_eq!(proportional(10, 1, 3, false).unwrap(), 3);
        assert!(proportional(10, 1, 0, false).is_err());
        assert!(proportional(u64::MAX, u64::MAX, 1, true).is_err());
    }

    #[test]
    fn swap_fee_and_invariant() {
        let (out, treasury) = quote(200_000_000, 200_000_000, 10_000_000, 30).unwrap();
        assert_eq!(treasury, 6_000);
        assert_eq!(out, (200_000_000u128 * 9_970_000 / 209_970_000) as u64);
        assert!(u128::from(210_000_000 - treasury) * u128::from(200_000_000 - out)
            >= 200_000_000u128.pow(2));
        assert!(quote(0, 1, 1, 30).is_err());
        assert!(quote(100, 100, 0, 30).is_err());
        assert!(quote(100, 100, 10, 10_000).is_err());
    }
}
