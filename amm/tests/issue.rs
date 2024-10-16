use amm::{
    constants::{
        CARROT_LOG_PROGRAM, CARROT_PROGRAM, CRT_MINT, CRT_VAULT, PYUSD_ORACLE, PYUSD_VAULT_ATA,
        USDC_MINT, USDC_ORACLE, USDC_VAULT_ATA, USDT_ORACLE, USDT_VAULT_ATA,
    },
    state::Vault,
    CarrotAmm,
};
use bincode::serialize;
use jupiter_amm_interface::{Amm, QuoteParams, SwapMode};
use solana_program_test::ProgramTest;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    program_pack::Pack,
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
async fn test_issue() {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);

    // add carrot programs
    program_test.add_program("carrot", CARROT_PROGRAM, None);
    program_test.add_program("carrot-log", CARROT_LOG_PROGRAM, None);

    // init account mapping
    let account_map = init_account_map();

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
                1_000_000_000_000,
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

    let amount = 1_000_000_000;

    let quote_params = QuoteParams {
        input_mint: USDC_MINT,
        output_mint: CRT_MINT,
        amount,
        swap_mode: SwapMode::ExactIn,
    };

    let quote = carrot_amm.quote(&quote_params).unwrap();
    assert_eq!(amount, quote.in_amount);

    let data = get_ix_data("issue", amount);

    let issue_ix = Instruction {
        program_id: CARROT_PROGRAM,
        accounts: vec![
            AccountMeta::new(CRT_VAULT, false),
            AccountMeta::new(CRT_MINT, false),
            AccountMeta::new(payer_shares_ata.pubkey(), false),
            AccountMeta::new(USDC_MINT, false),
            AccountMeta::new(USDC_VAULT_ATA, false),
            AccountMeta::new(payer_usdc_ata.pubkey(), false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(token_2022_program_id(), false),
            AccountMeta::new_readonly(CARROT_LOG_PROGRAM, false),
            AccountMeta::new_readonly(USDC_ORACLE, false),
            AccountMeta::new_readonly(USDT_ORACLE, false),
            AccountMeta::new_readonly(PYUSD_ORACLE, false),
            AccountMeta::new_readonly(USDC_VAULT_ATA, false),
            AccountMeta::new_readonly(USDT_VAULT_ATA, false),
            AccountMeta::new_readonly(PYUSD_VAULT_ATA, false),
        ],
        data,
    };

    let issue_tx = Transaction::new_signed_with_payer(
        &[issue_ix],
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
    assert_eq!(quote.out_amount, payer_shares_ata_data.amount);
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
