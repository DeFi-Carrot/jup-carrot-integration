use amm::constants::{CRT_MINT, CRT_VAULT, USDC_MINT};
use amm::{state::Vault, CarrotAmm};
use jupiter_amm_interface::{Amm, QuoteParams, SwapMode};

mod utils;
use rust_decimal::Decimal;
use utils::*;

#[tokio::test]
async fn test_quote_redeem_success() {
    // init account map from filesystem
    let account_map = load_account_map_from_file();

    // parse vault state
    let vault_account = account_map.get(&CRT_VAULT).unwrap();
    let vault_state: Vault = Vault::load(&vault_account.data).unwrap();

    // init amm
    let mut carrot_amm = CarrotAmm::new(CRT_VAULT, vault_state);

    // update related accounts, required before calling quote
    carrot_amm.update(&account_map).unwrap();

    let amount = 1_000;

    // Create QuoteParams for the test
    let quote_params = QuoteParams {
        input_mint: CRT_MINT,
        output_mint: USDC_MINT,
        amount,
        swap_mode: SwapMode::ExactIn,
    };

    // Call the quote method
    let quote_result = carrot_amm.quote(&quote_params).unwrap();
    assert_eq!(amount, quote_result.in_amount);
    assert_eq!(103, quote_result.out_amount);
    assert_eq!(1, quote_result.fee_amount);
    assert_eq!(Decimal::new(1, 4), quote_result.fee_pct);
    assert_eq!(CRT_MINT, quote_result.fee_mint);
    println!("{:?}", quote_result);
}

#[tokio::test]
async fn test_quote_redeem_insufficient_liquidity() {
    // init account map from filesystem
    let account_map = load_account_map_from_file();

    // parse vault state
    let vault_account = account_map.get(&CRT_VAULT).unwrap();
    let vault_state: Vault = Vault::load(&vault_account.data).unwrap();

    // init amm
    let mut carrot_amm = CarrotAmm::new(CRT_VAULT, vault_state);

    // update related accounts, required before calling quote
    carrot_amm.update(&account_map).unwrap();

    let amount = 1_000_000_000;

    // Create QuoteParams for the test
    let quote_params = QuoteParams {
        input_mint: CRT_MINT,
        output_mint: USDC_MINT,
        amount,
        swap_mode: SwapMode::ExactIn,
    };

    // Call the quote method
    let quote_result = carrot_amm.quote(&quote_params);
    assert!(
        quote_result.is_err(),
        "Expected an error, but got a successful result."
    );
}
