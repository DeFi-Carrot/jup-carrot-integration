use amm::constants::{CARROT_LOG_PROGRAM, CARROT_PROGRAM, CRT_MINT, USDC_MINT};
use solana_program_test::ProgramTest;
use solana_sdk::{
    program_pack::Pack,
    rent::Rent,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token::{id as token_program_id, state::Account as TokenAccount};
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

    let mint_authority = Keypair::from_bytes(&[
        6, 171, 218, 28, 81, 132, 195, 119, 106, 186, 21, 46, 6, 145, 196, 80, 151, 235, 245, 249,
        240, 102, 193, 29, 49, 156, 126, 163, 100, 6, 170, 23, 145, 253, 146, 149, 201, 100, 48,
        121, 249, 162, 172, 54, 190, 206, 106, 122, 68, 188, 49, 13, 252, 67, 233, 155, 72, 58, 62,
        174, 239, 185, 65, 165,
    ])
    .unwrap();

    let payer_shares_ata = Keypair::new();
    let payer_usdc_ata = Keypair::new();

    let transaction = Transaction::new_signed_with_payer(
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
                &payer.pubkey(),
                &[&mint_authority.pubkey()],
                1_000_000_000_000,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
        &[&payer, &mint_authority, &payer_shares_ata, &payer_usdc_ata],
        recent_blockhash,
    );

    banks_client.process_transaction(transaction).await.unwrap();
}
