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

pub fn calc_usd_amount(
    token_amount: u64,
    token_decimal: u8,
    price_feed_price: i64,
    price_feed_expo: i32,
    ceiling: bool,
) -> Option<u128> {
    if price_feed_expo >= 0 {
        return None;
    }

    let token_amount = token_amount as u128;
    let price_feed_price = price_feed_price.abs() as u128;

    // Scale the token amount to the base unit (USD cents)
    let scaled_token_amount =
        token_amount.checked_mul(10_u128.pow((PRECISION - token_decimal) as u32))?;

    // Perform safe multiplication to get numerator
    let numerator = scaled_token_amount.checked_mul(price_feed_price)?;

    let result = {
        let divisor = 10_u128.pow((-price_feed_expo) as u32);

        if ceiling {
            // Adjust for ceiling by adding divisor - 1 before division
            let adjusted_result = numerator.checked_add(divisor - 1)?.checked_div(divisor)?;
            Some(adjusted_result)
        } else {
            // Direct division for floor rounding
            let adjusted_result = numerator.checked_div(divisor)?;
            Some(adjusted_result)
        }
    };

    result
}

pub fn calc_token_amount(
    scaled_usd_amount: u128,
    token_decimal: u8,
    price_feed_price: i64,
    price_feed_expo: i32,
    ceiling: bool,
) -> Option<u64> {
    if price_feed_expo >= 0 {
        return None;
    }

    let price_feed_price = price_feed_price.abs() as u128;

    // Handle exponent adjustment for result based on the expo sign
    let result = {
        let multiplier = 10_u128.pow((-price_feed_expo) as u32);
        let temp_result = scaled_usd_amount.checked_mul(multiplier)?;
        let adjusted_result = if ceiling {
            temp_result.checked_add(price_feed_price - 1)?
        } else {
            temp_result
        };
        adjusted_result.checked_div(price_feed_price)
    }?;

    // Adjust for token decimals
    let divisor = 10_u128.pow((PRECISION - token_decimal) as u32);
    let token_amount = if ceiling {
        result.checked_add(divisor - 1)?.checked_div(divisor)
    } else {
        result.checked_div(divisor)
    }?;

    // Ensure the result fits within u64
    if token_amount > u64::MAX as u128 {
        None
    } else {
        Some(token_amount as u64)
    }
}

const PRECISION: u8 = 9;

fn ui_to_amount(ui: f64, decimal: u8) -> u64 {
    (ui * 10f64.powi(decimal as i32)) as u64
}
