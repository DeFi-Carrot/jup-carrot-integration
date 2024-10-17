use amm::{
    constants::{CARROT_LOG_PROGRAM, CARROT_PROGRAM, CRT_MINT, CRT_VAULT, USDC_MINT},
    state::Vault,
    CarrotAmm, CarrotSwap,
};
use bincode::serialize;
use jupiter_amm_interface::{Amm, QuoteParams, SwapMode};
use solana_program_test::ProgramTest;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token::{id as token_program_id, solana_program, state::Account as TokenAccount};
use spl_token_2022::{id as token_2022_program_id, state::Account as Token2022Account};

mod utils;
use utils::*;

#[tokio::test]
async fn test_issue_and_redeem() {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);

    // add carrot programs
    program_test.add_program("carrot", CARROT_PROGRAM, None);
    program_test.add_program("carrot-log", CARROT_LOG_PROGRAM, None);

    // init account mapping
    let mut account_map = load_account_map_from_file();

    // add all accounts to test harness
    for (address, account) in account_map.iter() {
        program_test.add_account(address.clone(), account.clone());
    }

    let rent = Rent::default();

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // use only for testing
    let mint_authority = Keypair::from_bytes(&[
        6, 171, 218, 28, 81, 132, 195, 119, 106, 186, 21, 46, 6, 145, 196, 80, 151, 235, 245, 249,
        240, 102, 193, 29, 49, 156, 126, 163, 100, 6, 170, 23, 145, 253, 146, 149, 201, 100, 48,
        121, 249, 162, 172, 54, 190, 206, 106, 122, 68, 188, 49, 13, 252, 67, 233, 155, 72, 58, 62,
        174, 239, 185, 65, 165,
    ])
    .unwrap();

    let payer_shares_ata = Keypair::new();
    let payer_usdc_ata = Keypair::new();

    let payer_usdc_mint_to = 1_000_000_000;

    let setup_payer_tx = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &payer_shares_ata.pubkey(),
                rent.minimum_balance(Token2022Account::LEN),
                Token2022Account::LEN as u64,
                &token_2022_program_id(),
            ),
            spl_token_2022::instruction::initialize_account3(
                &token_2022_program_id(),
                &payer_shares_ata.pubkey(),
                &CRT_MINT,
                &payer.pubkey(),
            )
            .unwrap(),
            system_instruction::create_account(
                &payer.pubkey(),
                &payer_usdc_ata.pubkey(),
                rent.minimum_balance(TokenAccount::LEN),
                TokenAccount::LEN as u64,
                &token_program_id(),
            ),
            spl_token::instruction::initialize_account3(
                &token_program_id(),
                &payer_usdc_ata.pubkey(),
                &USDC_MINT,
                &payer.pubkey(),
            )
            .unwrap(),
            spl_token::instruction::mint_to(
                &token_program_id(),
                &USDC_MINT,
                &payer_usdc_ata.pubkey(),
                &mint_authority.pubkey(),
                &[&mint_authority.pubkey()],
                payer_usdc_mint_to,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
        &[&payer, &mint_authority, &payer_shares_ata, &payer_usdc_ata],
        recent_blockhash,
    );

    banks_client
        .process_transaction_with_metadata(setup_payer_tx)
        .await
        .unwrap();

    // quote a issue operation with jup amm
    let vault_account = account_map.get(&CRT_VAULT).unwrap();
    let vault_state = Vault::load(&vault_account.data).unwrap();

    // init amm
    let mut carrot_amm = CarrotAmm::new(CRT_VAULT, vault_state);

    // update account cache
    carrot_amm.update(&account_map).unwrap();

    let usdc_amount = 1_000_000_000;

    let issue_quote_params = QuoteParams {
        input_mint: USDC_MINT,
        output_mint: CRT_MINT,
        amount: usdc_amount,
        swap_mode: SwapMode::ExactIn,
    };

    let issue_quote = carrot_amm.quote(&issue_quote_params).unwrap();
    assert_eq!(usdc_amount, issue_quote.in_amount);

    let issue_data = get_ix_data("issue", usdc_amount);

    let carrot_swap_issue = CarrotSwap {
        source_mint: issue_quote_params.input_mint,
        user_source: payer_usdc_ata.pubkey(),
        user_destination: payer_shares_ata.pubkey(),
        user_transfer_authority: payer.pubkey(),
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

    let issue_accounts: Vec<AccountMeta> = carrot_swap_issue.into();

    let issue_ix = Instruction {
        program_id: CARROT_PROGRAM,
        accounts: issue_accounts,
        data: issue_data,
    };

    let issue_tx = Transaction::new_signed_with_payer(
        &[compute_ix.clone(), issue_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    banks_client
        .process_transaction_with_metadata(issue_tx)
        .await
        .unwrap();

    // check how many shares were received
    let payer_shares = banks_client
        .get_account(payer_shares_ata.pubkey())
        .await
        .unwrap()
        .unwrap();
    let payer_shares_ata_data = Token2022Account::unpack(&payer_shares.data).unwrap();
    assert_eq!(issue_quote.out_amount, payer_shares_ata_data.amount);

    // fetch updated accounts as result of issue tx
    let account_map_addresses: Vec<Pubkey> = account_map.keys().cloned().collect();
    account_map =
        load_account_map_from_bank(&mut banks_client, account_map_addresses.as_slice()).await;

    // update amm with new account data
    carrot_amm.update(&account_map).unwrap();

    let crt_amount = 1_000_000_000;

    let redeem_quote_params = QuoteParams {
        input_mint: CRT_MINT,
        output_mint: USDC_MINT,
        amount: crt_amount,
        swap_mode: SwapMode::ExactIn,
    };

    let redeem_quote = carrot_amm.quote(&redeem_quote_params).unwrap();
    assert_eq!(crt_amount, redeem_quote.in_amount);

    let redeem_data = get_ix_data("redeem", crt_amount);

    let carrot_swap_redeem = CarrotSwap {
        source_mint: redeem_quote_params.input_mint,
        user_source: payer_shares_ata.pubkey(),
        user_destination: payer_usdc_ata.pubkey(),
        user_transfer_authority: payer.pubkey(),
    };

    let redeem_accounts: Vec<AccountMeta> = carrot_swap_redeem.into();

    let redeem_ix = Instruction {
        program_id: CARROT_PROGRAM,
        accounts: redeem_accounts,
        data: redeem_data,
    };

    let redeem_tx = Transaction::new_signed_with_payer(
        &[compute_ix, redeem_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    banks_client
        .process_transaction_with_metadata(redeem_tx)
        .await
        .unwrap();

    // assert usdc received for redemption
    let payer_usdc = banks_client
        .get_account(payer_usdc_ata.pubkey())
        .await
        .unwrap()
        .unwrap();
    let payer_usdc_ata_data = TokenAccount::unpack(&payer_usdc.data).unwrap();
    assert_eq!(redeem_quote.out_amount, payer_usdc_ata_data.amount);
}

fn get_function_hash(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);
    let mut sighash = [0u8; 8];
    sighash.copy_from_slice(&solana_program::hash::hash(preimage.as_bytes()).to_bytes()[..8]);
    sighash
}

fn get_ix_data(ix_name: &str, amount: u64) -> Vec<u8> {
    let hash = get_function_hash("global", ix_name);
    let mut buf: Vec<u8> = vec![];
    buf.extend_from_slice(&hash);
    let args = serialize(&amount).unwrap();
    buf.extend_from_slice(&args);
    buf
}
