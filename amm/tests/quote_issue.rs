use amm::constants::{CARROT_PROGRAM, CRT_MINT, CRT_VAULT, USDC_MINT};
use amm::{state::Vault, CarrotAmm};
use jupiter_amm_interface::{Amm, QuoteParams, SwapMode};

mod utils;
use utils::*;

#[tokio::test]
async fn test_quote_issue() {
    // init account map from filesystem
    let account_map = init_account_map();

    // parse vault state
    let vault_state: Vault = Vault::load(&account_map.get(&CRT_VAULT).unwrap().data).unwrap();

    // init
    let mut carrot_amm = CarrotAmm {
        label: "CarrotAmm".to_string(),
        program_id: CARROT_PROGRAM,
        vault: CRT_VAULT,
        vault_state,
        shares_state: None,
        asset_state: vec![],
    };

    // i think i need to call this
    carrot_amm.update(&account_map).unwrap();

    // Create QuoteParams for the test
    let quote_params = QuoteParams {
        input_mint: USDC_MINT,
        output_mint: CRT_MINT,
        amount: 1_000_000_000,
        swap_mode: SwapMode::ExactIn,
    };

    // Call the quote method
    let quote_result = carrot_amm.quote(&quote_params).unwrap();
    println!("{:?}", quote_result);
}
