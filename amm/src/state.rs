use solana_sdk::pubkey::Pubkey;

//
// accounts
//

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

pub struct Strategy {
    pub metadata: StrategyMetadata,
    pub strategy_type: StrategyType,
}

// data

pub struct Asset {
    pub asset_id: u16,
    pub mint: Pubkey,
    pub decimals: u8,
    pub ata: Pubkey,
    pub oracle: Pubkey,
}

pub struct StrategyRecord {
    pub strategy_id: u16,
    pub asset_id: u16,
    pub balance: u64,
    pub net_earnings: i64,
}

pub struct StrategyMetadata {
    pub name: String,
    pub strategy_id: u16,
    pub asset_mint: Pubkey,
    pub vault: Pubkey,
}

pub struct Fee {
    pub redemption_fee_bps: u16,
    pub redemption_fee_accumulated: u64,
    pub management_fee_bps: u16,
    pub management_fee_last_update: i64,
    pub management_fee_accumulated: u64,
    pub performance_fee_bps: u16,
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
