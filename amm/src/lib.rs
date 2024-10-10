use std::str::FromStr;

use anyhow::Result;
use jupiter_amm_interface::{
    try_get_account_data, AccountMap, Amm, AmmContext, KeyedAccount, Quote, QuoteParams, Swap,
    SwapAndAccountMetas, SwapParams,
};
use rust_decimal::Decimal;
use solana_sdk::{program_pack::Pack, pubkey::Pubkey};
use spl_token::state::Account as TokenAccount;

pub struct CarrotAmm {
    key: Pubkey,
    label: String,
    program_id: Pubkey,
    crt_mint: Pubkey,
    usdc_mint: Pubkey,
    usdc_mint_ata: Pubkey,
    reserves: [u64; 1],
    //usdt_mint: Pubkey,
    //pyusd_mint: Pubkey,
}

impl Amm for CarrotAmm {
    fn from_keyed_account(keyed_account: &KeyedAccount, _amm_context: &AmmContext) -> Result<Self> {
        Ok(CarrotAmm {
            key: keyed_account.key,
            label: "CarrotAmm".to_string(),
            program_id: keyed_account.account.owner,
            crt_mint: Pubkey::from_str("CRTx1JouZhzSU6XytsE42UQraoGqiHgxabocVfARTy2s").unwrap(),
            usdc_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            usdc_mint_ata: Pubkey::from_str("").unwrap(),
            reserves: [0],
            //usdt_mint: Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap(),
            //pyusd_mint: Pubkey::from_str("2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo").unwrap(),
        })
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    fn key(&self) -> Pubkey {
        self.key
    }

    fn get_reserve_mints(&self) -> Vec<Pubkey> {
        vec![
            self.crt_mint,
            self.usdc_mint,
            //self.usdt_mint,
            //self.pyusd_mint,
        ]
    }

    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        vec![self.key, self.usdc_mint_ata]
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {

        let usdc_mint_ata_data = try_get_account_data(account_map, &self.usdc_mint_ata)?;
        let usdc_mint_ata = TokenAccount::unpack(usdc_mint_ata_data)?;

        self.reserves = [
            usdc_mint_ata.amount.into(),
        ];

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        //let (trade_direction, swap_source_amount, swap_destination_amount) =
        //    if quote_params.input_mint == self.reserve_mints[0] {
        //        (TradeDirection::AtoB, self.reserves[0], self.reserves[1])
        //    } else {
        //        (TradeDirection::BtoA, self.reserves[1], self.reserves[0])
        //    };

        //let swap_result = get_swap_curve_result(
        //    &self.state.swap_curve,
        //    quote_params.amount,
        //    swap_source_amount,
        //    swap_destination_amount,
        //    trade_direction,
        //    &self.state.fees,
        //)?;

        Ok(Quote {
            fee_pct: Decimal::ZERO,
            in_amount: quote_params.amount,
            out_amount: quote_params.amount,
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

        let (swap_source, swap_destination) = if source_mint.eq(&self.crt_mint) {
            (self.state.token_a, self.state.token_b)
        } else {
            (self.state.token_b, self.state.token_a)
        };

        Ok(SwapAndAccountMetas {
            swap: Swap::TokenSwap,
            account_metas: TokenSwap {
                token_swap_program: self.program_id,
                token_program: spl_token::id(),
                swap: self.key,
                authority: self.get_authority(),
                user_transfer_authority: *token_transfer_authority,
                source: *source_token_account,
                destination: *destination_token_account,
                pool_mint: self.state.pool_mint,
                pool_fee: self.state.pool_fee_account,
                swap_destination,
                swap_source,
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
