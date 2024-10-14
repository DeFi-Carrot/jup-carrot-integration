use anyhow::Result;
use std::ops::Add;

//use pyth_solana_receiver_sdk::price_update::PriceUpdateV2;
use solana_sdk::{account_info::AccountInfo, pubkey::Pubkey};

use crate::calc_usd_amount;

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
pub struct Shares {
    pub mint: Pubkey,
    pub supply: u64,
    pub decimals: u8,
}

pub fn get_price_usd_from_pyth_oracle(
    oracle: &Pubkey,
    remaining_accounts: &[AccountInfo],
    rounding_mode: RoundingMode,
) -> (i64, i32) {
    // hardcode $1 for now
    (1_000_000_000, -9)

    //// find pyth oracle for asset
    //let pyth_oracle_account_info = remaining_accounts
    //    .iter()
    //    .find(|ra| ra.key.eq(&oracle))
    //    .unwrap();

    //let pyth_oracle_account_data = pyth_oracle_account_info.try_borrow_data()?;

    //let price_update = PriceUpdateV2::try_deserialize(&mut pyth_oracle_account_data.as_ref())?;

    //let price_feed_id = price_update.price_message.feed_id;

    //// just checking price staleness with this call
    //let _ = price_update.get_price_no_older_than(&Clock::get().unwrap(), 300, &price_feed_id)?;

    //// 5%
    //let max_confidence_threshold: u64 = 5_000_000;

    //// if confidence interval provided by pyth is outside our acceptable tolerance error
    //require!(
    //    price_update.price_message.ema_conf <= max_confidence_threshold,
    //    CarrotError::InvalidPriceConf
    //);

    //// adjust the price by the confidence value based on rounding mode
    //let adjusted_price = match rounding_mode {
    //    RoundingMode::RoundUp => price_update
    //        .price_message
    //        .ema_price
    //        .saturating_add(price_update.price_message.ema_conf as i64),
    //    RoundingMode::RoundDown => price_update
    //        .price_message
    //        .ema_price
    //        .saturating_sub(price_update.price_message.ema_conf as i64),
    //    RoundingMode::Avg => price_update.price_message.ema_price,
    //};

    //Ok((adjusted_price, price_update.price_message.exponent))
}

#[derive(Clone)]
pub enum RoundingMode {
    RoundUp,
    RoundDown,
    Avg,
}
