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
use spl_token::state::{Account as TokenAccount, GenericTokenAccount, Mint};
use spl_token_2022::{
    extension::StateWithExtensionsOwned,
    state::{Account as TokenAccount22, Mint as Mint22},
};

pub mod constants;
use constants::*;

mod math;
use math::*;
use state::{get_price_usd_from_pyth_oracle, AssetState, RoundingMode, Shares, Vault};

pub mod state;

pub struct CarrotAmm {
    pub label: String,
    pub program_id: Pubkey,
    pub vault: Pubkey,
    pub vault_state: Vault,
    pub shares_state: Option<Shares>,
    pub asset_state: Vec<AssetState>,
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

        Ok(CarrotAmm {
            label: "CarrotAmm".to_string(),
            program_id: keyed_account.account.owner,
            vault: keyed_account.key,
            vault_state,
            asset_state: vec![],
            shares_state: None,
        })
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
        self.shares_state = Some(Shares {
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

            // TODO: parse oracle account
            asset_state.push(AssetState {
                asset_id: asset.asset_id,
                mint: asset.mint,
                mint_decimals: asset.decimals,
                ata_amount,
                oracle_price: 1_000_000_000,
                oracle_price_expo: -9,
            });
        }
        self.asset_state = asset_state;

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        let vault_tvl = self.vault_state.get_tvl(&self.asset_state);

        let (out_amount, fee_pct) = if quote_params.input_mint.eq(&self.vault_state.shares) {
            // if input is shares, its a redemption operation
            let redeem_amount_usd = usd_earned(
                quote_params.amount,
                self.shares_state.unwrap().supply,
                vault_tvl,
            );

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

            (asset_amount, Decimal::new(1, 4))
        } else {
            // if input is not shares, its an issue operation
            let asset = self.vault_state.get_asset_by_mint(quote_params.input_mint);
            let (price, price_expo) =
                get_price_usd_from_pyth_oracle(&asset.oracle, &[], RoundingMode::Avg);
            let deposit_usd = calc_usd_amount(
                quote_params.amount,
                asset.decimals,
                price,
                price_expo,
                false,
            )
            .unwrap();

            // determine shares owed to depositor
            let shares_owed = shares_earned(
                deposit_usd,
                self.shares_state.unwrap().supply,
                self.shares_state.unwrap().decimals,
                vault_tvl,
                false,
            );

            (shares_owed, Decimal::ZERO)
        };

        let fee_amount = if fee_pct.is_zero() {
            0
        } else {
            // TODO: not exactly sure what to put here
            0
        };

        Ok(Quote {
            fee_pct,
            in_amount: quote_params.amount,
            out_amount,
            fee_amount,
            fee_mint: quote_params.input_mint,
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
