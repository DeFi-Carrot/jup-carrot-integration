use anyhow::{anyhow, Result};
use std::ops::Add;

//use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;
use solana_sdk::{clock::Clock, pubkey::Pubkey, sysvar::Sysvar};

use crate::{calc_usd_amount, shares_earned};

//
// accounts
//

#[derive(Clone, Debug)]
pub struct Vault {
    pub authority: Pubkey,
    pub shares: Pubkey,
    pub fee: Fee,
    pub paused: bool,
    pub asset_index: u16,
    pub strategy_index: u16,
    pub assets: Vec<Asset>,
    pub strategies: Vec<StrategyRecord>,
}

impl Vault {
    pub fn load(account_data: &[u8]) -> Result<Self> {
        let mut offset = 8; // start at 8 to skip anchor account discriminator

        // Read fixed size fields
        let authority = Pubkey::new_from_array(account_data[offset..offset + 32].try_into()?);
        offset += 32;

        let shares = Pubkey::new_from_array(account_data[offset..offset + 32].try_into()?);
        offset += 32;

        // Assuming Fee::load exists and correctly handles deserialization
        let fee = Fee::load(&account_data[offset..offset + Fee::SPACE])?;
        offset += Fee::SPACE;

        let paused = account_data[offset] > 0;
        offset += 1;

        let asset_index = u16::from_le_bytes(account_data[offset..offset + 2].try_into()?);
        offset += 2;

        let strategy_index = u16::from_le_bytes(account_data[offset..offset + 2].try_into()?);
        offset += 2;

        // Dynamic Vec<Asset> deserialization
        let assets_len = u32::from_le_bytes(account_data[offset..offset + 4].try_into()?);
        offset += 4;
        let mut assets = Vec::with_capacity(assets_len as usize);
        for _ in 0..assets_len {
            let asset = Asset::load(&account_data[offset..offset + Asset::SPACE])?;
            assets.push(asset);
            offset += Asset::SPACE;
        }

        // Dynamic Vec<StrategyRecord> deserialization
        let strategies_len = u32::from_le_bytes(account_data[offset..offset + 4].try_into()?);
        offset += 4;
        let mut strategies = Vec::with_capacity(strategies_len as usize);
        for _ in 0..strategies_len {
            let strategy =
                StrategyRecord::load(&account_data[offset..offset + StrategyRecord::SPACE])?;
            strategies.push(strategy);
            offset += StrategyRecord::SPACE;
        }

        Ok(Vault {
            authority,
            shares,
            fee,
            paused,
            asset_index,
            strategy_index,
            assets,
            strategies,
        })
    }

    // get total vault balance in usd
    // looks at strategy balances and ATA balances
    pub fn get_tvl(&self, asset_state: &Vec<AssetState>) -> u128 {
        let total_strategy_balance: u128 = self
            .strategies
            .iter()
            .map(|strat| {
                let state = asset_state
                    .iter()
                    .find(|a| a.asset_id.eq(&strat.asset_id))
                    .unwrap();
                let balance_usd = strat.get_balance_usd(state);

                balance_usd
            })
            .collect::<Vec<u128>>()
            .iter()
            .sum();

        let total_reserve_balance: u128 = self
            .assets
            .iter()
            .map(|asset| {
                let state = asset_state
                    .iter()
                    .find(|a| a.asset_id.eq(&asset.asset_id))
                    .unwrap();
                let balance_usd = asset.get_balance_usd(state);
                balance_usd
            })
            .collect::<Vec<u128>>()
            .iter()
            .sum();

        total_strategy_balance.add(total_reserve_balance)
    }

    pub fn calculate_accumulated_performance_fee(
        &self,
        asset_state: &Vec<AssetState>,
        shares_supply: u64,
        shares_decimals: u8,
        vault_tvl: u128,
    ) -> Result<u64> {
        let mut performance_fee_accumulated: u64 = 0;
        for strategy in self.strategies.iter() {
            // find strategy asset
            let asset = asset_state
                .iter()
                .find(|a| a.asset_id.eq(&strategy.asset_id))
                .unwrap();

            // calculate performance fee for each strategy
            let strategy_performance_fee = self.fee.calculate_performance_fee(
                strategy.net_earnings,
                asset.oracle_price,
                asset.oracle_price_expo,
                asset.mint_decimals,
                shares_supply,
                shares_decimals,
                vault_tvl,
            );
            performance_fee_accumulated += strategy_performance_fee;
        }

        Ok(performance_fee_accumulated)
    }

    pub fn get_asset_by_mint(&self, asset_mint: Pubkey) -> &Asset {
        self.assets.iter().find(|a| a.mint.eq(&asset_mint)).unwrap()
    }
}

pub struct Strategy {
    pub metadata: StrategyMetadata,
    pub strategy_type: StrategyType,
}

// data

#[derive(Clone, Copy, Debug)]
pub struct Asset {
    pub asset_id: u16,
    pub mint: Pubkey,
    pub decimals: u8,
    pub ata: Pubkey,
    pub oracle: Pubkey,
}

impl Asset {
    pub const SPACE: usize = 2 + 32 + 1 + 32 + 32;

    pub fn load(account_data: &[u8]) -> Result<Self> {
        assert_eq!(account_data.len(), Self::SPACE);

        let asset_id = u16::from_le_bytes(account_data[0..2].try_into()?);
        let mint = Pubkey::new_from_array(account_data[2..34].try_into()?);
        let decimals = account_data[34];
        let ata = Pubkey::new_from_array(account_data[35..67].try_into()?);
        let oracle = Pubkey::new_from_array(account_data[67..99].try_into()?);

        Ok(Asset {
            asset_id,
            mint,
            decimals,
            ata,
            oracle,
        })
    }

    fn get_balance_usd(&self, asset_state: &AssetState) -> u128 {
        let balance_usd = calc_usd_amount(
            asset_state.ata_amount,
            asset_state.mint_decimals,
            asset_state.oracle_price,
            asset_state.oracle_price_expo,
            true,
        )
        .unwrap();

        balance_usd
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StrategyRecord {
    pub strategy_id: u16,
    pub asset_id: u16,
    pub balance: u64,
    pub net_earnings: i64,
}

impl StrategyRecord {
    pub const SPACE: usize = 2 + 2 + 8 + 8;

    pub fn load(account_data: &[u8]) -> Result<Self> {
        assert_eq!(account_data.len(), Self::SPACE);

        let strategy_id = u16::from_le_bytes(account_data[0..2].try_into()?);
        let asset_id = u16::from_le_bytes(account_data[2..4].try_into()?);
        let balance = u64::from_le_bytes(account_data[4..12].try_into()?);
        let net_earnings = i64::from_le_bytes(account_data[12..20].try_into()?);

        Ok(StrategyRecord {
            strategy_id,
            asset_id,
            balance,
            net_earnings,
        })
    }

    fn get_balance_usd(&self, asset_state: &AssetState) -> u128 {
        let balance_usd = calc_usd_amount(
            self.balance,
            asset_state.mint_decimals,
            asset_state.oracle_price,
            asset_state.oracle_price_expo,
            true,
        )
        .unwrap();
        balance_usd
    }
}

pub struct StrategyMetadata {
    pub name: String,
    pub strategy_id: u16,
    pub asset_mint: Pubkey,
    pub vault: Pubkey,
}

#[derive(Clone, Copy, Debug)]
pub struct Fee {
    pub redemption_fee_bps: u16,
    pub redemption_fee_accumulated: u64,
    pub management_fee_bps: u16,
    pub management_fee_last_update: i64,
    pub management_fee_accumulated: u64,
    pub performance_fee_bps: u16,
}

impl Fee {
    // Assuming the SPACE constant for Fee is defined as the sum of its fields' sizes
    pub const SPACE: usize = 2 + 8 + 2 + 8 + 8 + 2; // Example, adjust based on actual sizes

    const SECONDS_IN_YEAR: f64 = 31557600.0;

    pub fn load(account_data: &[u8]) -> Result<Self> {
        assert_eq!(account_data.len(), Self::SPACE);

        let mut offset = 0;

        // Deserialize each field from the byte slice
        let redemption_fee_bps = u16::from_le_bytes(account_data[offset..offset + 2].try_into()?);
        offset += 2;

        let redemption_fee_accumulated =
            u64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let management_fee_bps = u16::from_le_bytes(account_data[offset..offset + 2].try_into()?);
        offset += 2;

        let management_fee_last_update =
            i64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let management_fee_accumulated =
            u64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let performance_fee_bps = u16::from_le_bytes(account_data[offset..offset + 2].try_into()?);
        // No need to adjust offset here if it's the last field

        Ok(Fee {
            redemption_fee_bps,
            redemption_fee_accumulated,
            management_fee_bps,
            management_fee_last_update,
            management_fee_accumulated,
            performance_fee_bps,
        })
    }

    // returns the number of shares that should be minted to the fee account
    // increments accumulated store
    pub fn calculate_management_fee(
        &self,
        tvl: u128,
        shares_supply: u64,
        shares_decimals: u8,
    ) -> u64 {
        let current_time = Clock::get().unwrap().unix_timestamp;

        // require a delta of over 60 seconds
        let time_delta = current_time - self.management_fee_last_update;
        if time_delta <= 60 {
            return 0;
        }

        // Calculate elapsed time in seconds
        let elapsed_seconds = time_delta as u128;

        // Calculate fee in USD cents using integer arithmetic
        let fee_usd_cents = (self.calc_management_fee(tvl) as u128 * elapsed_seconds)
            / Fee::SECONDS_IN_YEAR as u128;

        //// update timestamp
        //self.management_fee_last_update = current_time;

        if fee_usd_cents == 0 {
            return 0;
        }

        // convert usd cents to shares ui based on NAV
        let shares_amount = shares_earned(fee_usd_cents, shares_supply, shares_decimals, tvl, true);

        //// increment accumulated store
        //self.management_fee_accumulated += shares_amount;

        shares_amount
    }

    // returns the shares amount of the performance fee that should be minted to the fee account
    pub fn calculate_performance_fee(
        &self,
        net_earnings: i64,
        asset_price: i64,
        asset_price_expo: i32,
        asset_decimals: u8,
        shares_supply: u64,
        shares_decimals: u8,
        vault_tvl: u128,
    ) -> u64 {
        // if we lost/didnt make any money dont charge a fee
        if net_earnings.le(&0) {
            return 0;
        };

        // calculate value of earnings in usd
        let net_earnings_usd = calc_usd_amount(
            net_earnings as u64,
            asset_decimals,
            asset_price,
            asset_price_expo,
            true,
        )
        .unwrap();

        // calculate performance fee in usd
        let fee_amount_usd = self.calc_performance_fee(net_earnings_usd);

        let fee_amount_shares = shares_earned(
            fee_amount_usd,
            shares_supply,
            shares_decimals,
            vault_tvl,
            true,
        );

        fee_amount_shares
    }

    // returns (remaining_amount after fee, fee_amount)
    pub fn calculate_redemption_fee(&self, redemption_amount: u64) -> (u64, u64) {
        if self.redemption_fee_bps == 0 {
            return (redemption_amount, 0);
        }

        let (remaining_amount, fee_amount) = self.calc_redemption_fee(redemption_amount);
        //self.redemption_fee_accumulated += fee_amount;

        return (remaining_amount, fee_amount);
    }

    // inflates the shares_supply by the amount of unrealized fees accrued by the protocol
    // performance fees is computed inside the ix, which is why we pass it in
    pub fn adjust_shares_by_fees(
        &self,
        shares_supply: u64,
        total_performance_fees_accumulated: u64,
    ) -> u64 {
        shares_supply
            + total_performance_fees_accumulated
            + self.management_fee_accumulated
            + self.redemption_fee_accumulated
    }

    fn calc_management_fee(&self, tvl: u128) -> u128 {
        ((tvl * self.management_fee_bps as u128 + 9_999) / 10_000) // round up
            .try_into()
            .unwrap()
    }

    fn calc_performance_fee(&self, net_earnings_usd: u128) -> u128 {
        (net_earnings_usd * self.performance_fee_bps as u128 + 9_999) / 10_000 // round up
    }

    // return (remaining redemption amount after fee, fee amount taken)
    fn calc_redemption_fee(&self, redemption_amount: u64) -> (u64, u64) {
        let fee_amount = (redemption_amount * self.redemption_fee_bps as u64 + 9_999) / 10_000; // round up
        let remaining_amount = redemption_amount - fee_amount;
        (remaining_amount, fee_amount)
    }
}

pub enum StrategyType {
    MarginfiSupply {
        account: Pubkey,
        group: Pubkey,
        bank: Pubkey,
        bank_liquidity_vault: Pubkey,
        bank_liquidity_vault_authority: Pubkey,
        oracle: Pubkey,
    },
    KlendSupply {
        reserve: Pubkey,
        reserve_collateral_mint: Pubkey,
        reserve_liquidity_supply: Pubkey,
        reserve_destination_deposit_collateral: Pubkey,
        reserve_farm_state: Pubkey,
        lending_market: Pubkey,
        oracle: Pubkey,
        scope_prices: Pubkey,
    },
    SolendSupply {
        reserve: Pubkey,
        reserve_collateral_mint: Pubkey,
        reserve_liquidity_supply: Pubkey,
        deposit_collateral_ata: Pubkey,
        lending_market: Pubkey,
        lending_market_authority: Pubkey,
        pyth_oracle: Pubkey,
        switchboard_oracle: Pubkey,
    },
    MangoSupply {
        group: Pubkey,
        account: Pubkey,
        bank: Pubkey,
        vault: Pubkey,
        pyth_oracle: Pubkey,
        switchboard_oracle: Pubkey,
    },
    DriftSupply {
        state: Pubkey,
        signer: Pubkey,
        spot_market: Pubkey,
        spot_market_vault: Pubkey,
        perp_market: Pubkey,
        spot_pyth_oracle: Pubkey,
        perp_pyth_oracle: Pubkey,
        sub_account_id: u16,
        market_index: u16,
    },
    DriftInsuranceFund {
        state: Pubkey,
        spot_market: Pubkey,
        spot_market_vault: Pubkey,
        market_index: u16,
    },
}

#[derive(Clone, Copy)]
pub struct AssetState {
    pub asset_id: u16,
    pub mint: Pubkey,
    pub mint_decimals: u8,
    pub ata_amount: u64,
    pub oracle_price: i64,
    pub oracle_price_expo: i32,
}

#[derive(Clone, Copy)]
pub struct SharesState {
    pub mint: Pubkey,
    pub supply: u64,
    pub decimals: u8,
}

// pyth price account
// manually copied and parsed because of dependency issues with pyth rust crate
#[derive(Clone, Copy)]
pub struct PriceUpdateV2 {
    pub write_authority: Pubkey,
    pub verification_level: VerificationLevel,
    pub price_message: PriceFeedMessage,
    pub posted_slot: u64,
}

impl PriceUpdateV2 {
    pub const SPACE: usize = 8 + 32 + 2 + 32 + 8 + 8 + 4 + 8 + 8 + 8 + 8 + 8;

    pub fn load(account_data: &[u8]) -> Result<Self> {
        assert_eq!(account_data.len(), Self::SPACE);
        let mut offset = 8;

        let write_authority = Pubkey::new_from_array(account_data[offset..offset + 32].try_into()?);
        offset += 32;

        // parse verification level
        let verification_byte = account_data[offset];
        offset += 1; // Move past the verification level byte

        let verification_level = match verification_byte {
            0x01 => VerificationLevel::Full,
            0x00 => {
                // If Partial, assume the next byte indicates the number of signatures
                let num_signatures = account_data[offset];
                offset += 1; // Move past the num_signatures byte
                VerificationLevel::Partial { num_signatures }
            }
            _ => return Err(anyhow!("Unknown verification level byte")),
        };

        let feed_id = account_data[offset..offset + 32].try_into()?;
        offset += 32;

        let price = i64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let conf = u64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let exponent = i32::from_le_bytes(account_data[offset..offset + 4].try_into()?);
        offset += 4;

        let publish_time = i64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let prev_publish_time = i64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let ema_price = i64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let ema_conf = u64::from_le_bytes(account_data[offset..offset + 8].try_into()?);
        offset += 8;

        let posted_slot = u64::from_le_bytes(account_data[offset..offset + 8].try_into()?);

        Ok(PriceUpdateV2 {
            write_authority,
            verification_level,
            price_message: PriceFeedMessage {
                feed_id,
                price,
                conf,
                exponent,
                publish_time,
                prev_publish_time,
                ema_price,
                ema_conf,
            },
            posted_slot,
        })
    }

    // Updated get_price_usd_from_pyth_oracle function
    pub fn get_price_usd_from_pyth_oracle(&self, rounding_mode: RoundingMode) -> (i64, i32) {
        // Adjust the price by the confidence value based on rounding mode
        let adjusted_price = match rounding_mode {
            RoundingMode::RoundUp => self
                .price_message
                .ema_price
                .saturating_add(self.price_message.ema_conf as i64),
            RoundingMode::RoundDown => self
                .price_message
                .ema_price
                .saturating_sub(self.price_message.ema_conf as i64),
            RoundingMode::Avg => self.price_message.ema_price,
        };

        (adjusted_price, self.price_message.exponent)
    }
}

#[derive(Clone, Copy)]
pub enum VerificationLevel {
    Partial { num_signatures: u8 },
    Full,
}

#[derive(Clone, Copy)]
pub struct PriceFeedMessage {
    pub feed_id: FeedId,
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    pub publish_time: i64,
    pub prev_publish_time: i64,
    pub ema_price: i64,
    pub ema_conf: u64,
}

pub type FeedId = [u8; 32];

#[derive(Clone)]
pub enum RoundingMode {
    RoundUp,
    RoundDown,
    Avg,
}
