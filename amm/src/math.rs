// calculate the shares earned from depositing the usd
pub fn shares_earned(
    usd_value: u128,
    shares_supply: u64,
    shares_decimals: u8,
    vault_tvl: u128,
    round_up: bool,
) -> u64 {
    if vault_tvl.le(&0) || shares_supply.le(&0) {
        // if vault_tvl or shares_supply is 0 or less, just mint 100 shares
        ui_to_amount(100.0, shares_decimals)
    } else {
        // Calculate shares
        let shares = if round_up {
            (usd_value * shares_supply as u128 + vault_tvl - 1) / vault_tvl
        } else {
            usd_value * shares_supply as u128 / vault_tvl
        };

        // Check if the result fits within u64
        if shares > u64::MAX as u128 {
            u64::MAX // Handle overflow case, e.g., by returning the maximum u64 value
        } else {
            shares as u64
        }
    }
}

pub fn usd_earned(shares_to_redeem: u64, shares_supply: u64, vault_tvl: u128) -> u128 {
    if vault_tvl.le(&0) || shares_supply.le(&0) {
        // if vault_tvl or shares_supply is 0 or less, return 0 USD
        0
    } else {
        // rounds down
        let usd_earned = shares_to_redeem as u128 * vault_tvl / shares_supply as u128;

        usd_earned
    }
}

fn ui_to_amount(ui: f64, decimal: u8) -> u64 {
    (ui * 10f64.powi(decimal as i32)) as u64
}
