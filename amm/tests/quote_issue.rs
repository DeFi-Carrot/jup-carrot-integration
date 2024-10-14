use amm::constants::{CARROT_PROGRAM, CRT_MINT, CRT_VAULT, USDC_MINT};
use amm::state::{AssetState, Shares, Vault};
use amm::CarrotAmm;
use base64::{engine::general_purpose::STANDARD, Engine};
use jupiter_amm_interface::Amm;
use jupiter_amm_interface::{QuoteParams, SwapMode};
use rust_decimal::Decimal;
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

mod utils;
use utils::*;

#[tokio::test]
async fn test_quote_issue() {
    // read json file representing vault account
    let path = Path::new("tests/fixtures/vault.json");
    let mut file = File::open(&path).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    // deserialize json into generic value
    let parsed_json: Value = serde_json::from_str(&data).unwrap();

    // decode base64 data field
    let base64_data = STANDARD
        .decode(parsed_json["account"]["data"][0].as_str().unwrap())
        .unwrap();

    // load it
    let vault_state = Vault::load(&base64_data).unwrap();

    // Initialize CarrotAmm with dummy data
    let mut carrot_amm = CarrotAmm {
        label: "CarrotAmm".to_string(),
        program_id: CARROT_PROGRAM,
        vault: CRT_VAULT,
        vault_state: vault_state,
        shares_state: Some(Shares {
            mint: CRT_MINT,
            supply: 1_000_000_000_000,
            decimals: 9,
        }),
        asset_state: vec![AssetState {
            asset_id: 0,
            mint: USDC_MINT,
            mint_decimals: 6,
            ata_amount: 1_000_000_000_000_000,
            oracle_price: 1_000_000_000,
            oracle_price_expo: -9,
        }],
    };

    // Create QuoteParams for the test
    let quote_params = QuoteParams {
        input_mint: USDC_MINT,
        output_mint: CRT_MINT,
        amount: 1_000_000_000,
        swap_mode: SwapMode::ExactIn,
    };

    // init account map from filesystem
    let account_map = init_account_map();

    // i think i need to call this
    carrot_amm.update(&account_map).unwrap();

    // Call the quote method
    let quote_result = carrot_amm.quote(&quote_params);

    // Assert the expected outcome
    assert!(quote_result.is_ok());
    let quote = quote_result.unwrap();
    println!("quote: {:#?}", quote);
}
