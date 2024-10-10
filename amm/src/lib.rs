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
use spl_token::state::{Account as TokenAccount, Mint};

mod constants;
use constants::*;

mod math;
use math::*;

pub struct CarrotAmm {
    vault: Pubkey,
    label: String,
    program_id: Pubkey,
    crt_mint: Pubkey,
    usdc_mint: Pubkey,
    usdc_mint_ata: Pubkey,
    reserves: [u64; 2],
}

impl Clone for CarrotAmm {
    fn clone(&self) -> Self {
        CarrotAmm {
            vault: self.vault,
            label: self.label.clone(),
            program_id: self.program_id,
            crt_mint: self.crt_mint,
            usdc_mint: self.usdc_mint,
            usdc_mint_ata: self.usdc_mint_ata,
            reserves: self.reserves,
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
        Ok(CarrotAmm {
            vault: keyed_account.key,
            label: "CarrotAmm".to_string(),
            program_id: keyed_account.account.owner,
            crt_mint: CRT_MINT,
            usdc_mint: USDC_MINT,
            usdc_mint_ata: USDC_VAULT_ATA,
            reserves: [0, 0],
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
        vec![self.crt_mint, self.usdc_mint]
    }

    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        vec![self.vault, self.usdc_mint_ata, self.crt_mint]
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        let usdc_mint_ata_data = try_get_account_data(account_map, &self.usdc_mint_ata)?;
        let usdc_mint_ata = TokenAccount::unpack(usdc_mint_ata_data)?;

        let crt_mint_data = try_get_account_data(account_map, &self.crt_mint)?;
        let crt_mint = Mint::unpack(crt_mint_data)?;

        self.reserves = [usdc_mint_ata.amount.into(), crt_mint.supply.into()];

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        let fee_pct = if quote_params.input_mint.eq(&USDC_MINT) {
            // issue
            Decimal::ZERO
        } else {
            // redeem
            // only a fee to redeem
            Decimal::new(1, 4) // 0.01%
        };

        Ok(Quote {
            fee_pct,
            in_amount: quote_params.amount,
            out_amount: 0,
            fee_amount: 0,
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
