use anyhow::Result;
use constants::{CRT_MINT, CRT_VAULT};
use jupiter_amm_interface::{
    try_get_account_data, AccountMap, Amm, AmmContext, KeyedAccount, Quote, QuoteParams, Swap,
    SwapAndAccountMetas, SwapParams,
};
use rust_decimal::Decimal;
use solana_sdk::{
    instruction::AccountMeta, program_pack::Pack, pubkey::Pubkey,
    system_program::ID as SystemProgramId,
};
use spl_token::state::Account as TokenAccount;
use spl_token_2022::{
    extension::StateWithExtensionsOwned,
    state::{Account as TokenAccount22, Mint as Mint22},
};

pub mod constants;
use constants::*;

mod math;
use math::*;
use state::{AssetState, PriceUpdateV2, SharesState, Vault};

pub mod state;

pub struct CarrotAmm {
    pub label: String,
    pub program_id: Pubkey,
    pub vault: Pubkey,
    pub vault_state: Vault,
    pub shares_state: Option<SharesState>,
    pub asset_state: Vec<AssetState>,
}

impl CarrotAmm {
    pub fn new(vault: Pubkey, vault_state: Vault) -> Self {
        CarrotAmm {
            label: AMM_LABEL.to_owned(),
            program_id: CARROT_PROGRAM,
            vault,
            vault_state,
            asset_state: vec![],
            shares_state: None,
        }
    }
}

impl Clone for CarrotAmm {
    fn clone(&self) -> Self {
        CarrotAmm {
            label: self.label.clone(),
            program_id: self.program_id,
            vault: self.vault,
            vault_state: self.vault_state.clone(),
            asset_state: self.asset_state.clone(),
            shares_state: self.shares_state,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CarrotSwap {
    pub source_mint: Pubkey,
    pub user_source: Pubkey,
    pub user_destination: Pubkey,
    pub user_transfer_authority: Pubkey,
}

impl From<CarrotSwap> for Vec<AccountMeta> {
    fn from(accounts: CarrotSwap) -> Self {
        let (source_account, destination_account) = if accounts.source_mint.eq(&USDC_MINT) {
            (accounts.user_source, accounts.user_destination)
        } else {
            (accounts.user_destination, accounts.user_source)
        };

        let mut account_metas = vec![
            AccountMeta::new(CARROT_PROGRAM, false),
            AccountMeta::new(CRT_VAULT, false),
            AccountMeta::new(CRT_MINT, false),
            AccountMeta::new(destination_account, false),
            AccountMeta::new_readonly(USDC_MINT, false),
            AccountMeta::new(USDC_VAULT_ATA, false),
            AccountMeta::new(source_account, true),
            AccountMeta::new(accounts.user_transfer_authority, false),
            AccountMeta::new(SystemProgramId, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM, false),
            AccountMeta::new_readonly(TOKEN_22_PROGRAM, false),
            AccountMeta::new_readonly(CARROT_LOG_PROGRAM, false),
        ];

        // Add remaining accounts depending on assets the vault holds
        account_metas.extend_from_slice(&[
            AccountMeta::new_readonly(USDC_VAULT_ATA, false),
            AccountMeta::new_readonly(USDC_ORACLE, false),
            AccountMeta::new_readonly(USDT_VAULT_ATA, false),
            AccountMeta::new_readonly(USDT_ORACLE, false),
            AccountMeta::new_readonly(PYUSD_VAULT_ATA, false),
            AccountMeta::new_readonly(PYUSD_ORACLE, false),
        ]);

        account_metas
    }
}

impl Amm for CarrotAmm {
    fn from_keyed_account(keyed_account: &KeyedAccount, _amm_context: &AmmContext) -> Result<Self> {
        let vault_state = Vault::load(&keyed_account.account.data)?;

        Ok(CarrotAmm::new(keyed_account.key, vault_state))
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn key(&self) -> Pubkey {
        self.vault
    }

    fn get_reserve_mints(&self) -> Vec<Pubkey> {
        vec![CRT_MINT, USDC_MINT, USDT_MINT, PYUSD_MINT]
    }

    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        // add vault and shares mint
        let mut accounts = vec![self.vault, self.vault_state.shares];

        // add all assets
        for a in self.vault_state.assets.iter() {
            accounts.extend(vec![a.ata, a.oracle]);
        }

        // TODO: add strategy accounts
        accounts
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        // update vault state
        let vault_data = try_get_account_data(account_map, &self.vault)?;
        let vault = Vault::load(vault_data)?;
        self.vault_state = vault;

        // update shares state
        let mint_data = try_get_account_data(account_map, &self.vault_state.shares)?;
        let mint = StateWithExtensionsOwned::<Mint22>::unpack(mint_data.to_vec()).unwrap();
        self.shares_state = Some(SharesState {
            mint: self.vault_state.shares,
            supply: mint.base.supply,
            decimals: mint.base.decimals,
        });

        // update state for vault assets
        let mut asset_state: Vec<AssetState> = Vec::with_capacity(self.vault_state.assets.len());
        for asset in self.vault_state.assets.iter() {
            let ata_data = try_get_account_data(account_map, &asset.ata)?;

            // try to parse first as regular spl-token and if that errors try spl-token-2022
            let ata_amount = match TokenAccount::unpack(ata_data) {
                Ok(ata) => ata.amount,
                Err(_) => {
                    let ata =
                        StateWithExtensionsOwned::<TokenAccount22>::unpack(ata_data.to_vec())?;
                    ata.base.amount
                }
            };

            // parse pyth oracle data
            let oracle_data = try_get_account_data(account_map, &asset.oracle)?;
            let oracle = PriceUpdateV2::load(oracle_data)?;

            asset_state.push(AssetState {
                asset_id: asset.asset_id,
                mint: asset.mint,
                mint_decimals: asset.decimals,
                ata_amount,
                oracle_price: oracle.price_message.price,
                oracle_price_expo: oracle.price_message.exponent,
            });
        }
        self.asset_state = asset_state;

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        let vault_tvl = self.vault_state.get_tvl(&self.asset_state);

        let shares_state = self.shares_state.unwrap();

        // calculate unminted performance fees, used to adjust the shares supply
        let accumulated_performance_fee = self.vault_state.calculate_accumulated_performance_fee(
            &self.asset_state,
            shares_state.supply,
            shares_state.decimals,
            vault_tvl,
        )?;

        // adjust shares supply by unminted fees accrued
        // this is just used to have an accurate supply to calculate the management fee
        let adjusted_shares_supply_before_mgmt_fee = self
            .vault_state
            .fee
            .adjust_shares_by_fees(shares_state.supply, accumulated_performance_fee);

        // calculate management fee before deposit
        // TODO: figure out how to test this
        let _fee_amount = self.vault_state.fee.calculate_management_fee(
            vault_tvl,
            adjusted_shares_supply_before_mgmt_fee,
            shares_state.decimals,
        );

        // adjust shares supply by unminted fees accrued
        // this is now the true adjusted shares supply because it takes into account the latest fee data
        let adjusted_shares_supply = self
            .vault_state
            .fee
            .adjust_shares_by_fees(shares_state.supply, accumulated_performance_fee);

        let (out_amount, fee_pct, fee_amount) =
            if quote_params.input_mint.eq(&self.vault_state.shares) {
                // calculate redemption fee
                let (adjusted_after_redemption_fee_amount, redemption_fee_amount) = self
                    .vault_state
                    .fee
                    .calculate_redemption_fee(quote_params.amount);

                // if input is shares, its a redemption operation
                let redeem_amount_usd = usd_earned(
                    quote_params.amount,
                    adjusted_after_redemption_fee_amount,
                    vault_tvl,
                );

                // TODO make this a method
                let asset_state = self
                    .asset_state
                    .iter()
                    .find(|a| a.mint.eq(&quote_params.output_mint))
                    .unwrap();

                let asset_amount = calc_token_amount(
                    redeem_amount_usd,
                    asset_state.mint_decimals,
                    asset_state.oracle_price,
                    asset_state.oracle_price_expo,
                    false,
                )
                .unwrap();

                (
                    asset_amount,
                    Decimal::new(self.vault_state.fee.redemption_fee_bps.into(), 4),
                    redemption_fee_amount,
                )
            } else {
                // if input is not shares, its an issue operation
                let asset = self
                    .asset_state
                    .iter()
                    .find(|a| a.mint.eq(&quote_params.input_mint))
                    .unwrap();
                let deposit_usd = calc_usd_amount(
                    quote_params.amount,
                    asset.mint_decimals,
                    asset.oracle_price,
                    asset.oracle_price_expo,
                    false,
                )
                .unwrap();

                // determine shares owed to depositor
                let shares_owed = shares_earned(
                    deposit_usd,
                    adjusted_shares_supply,
                    shares_state.decimals,
                    vault_tvl,
                    false,
                );

                // TODO: 0 should probably be a variable when i write tests for manage/perf fees even though they're currently disabled
                (shares_owed, Decimal::ZERO, 0)
            };

        Ok(Quote {
            fee_pct,
            in_amount: quote_params.amount,
            out_amount,
            fee_amount,
            fee_mint: CRT_MINT,
            ..Quote::default()
        })
    }

    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas> {
        let SwapParams {
            token_transfer_authority,
            source_token_account,
            destination_token_account,
            source_mint,
            ..
        } = swap_params;

        Ok(SwapAndAccountMetas {
            swap: Swap::TokenSwap,
            account_metas: CarrotSwap {
                user_destination: *destination_token_account,
                user_source: *source_token_account,
                user_transfer_authority: *token_transfer_authority,
                source_mint: *source_mint,
            }
            .into(),
        })
    }

    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }

    fn supports_exact_out(&self) -> bool {
        false
    }
}
