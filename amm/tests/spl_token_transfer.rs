use solana_program_test::{processor, ProgramTest};
use solana_sdk::{
    program_pack::Pack, rent::Rent, signature::Signer, signer::keypair::Keypair,
    system_instruction, transaction::Transaction,
};
use spl_token::{
    id as token_program_id,
    processor::Processor,
    state::{Account as TokenAccount, Mint},
};

#[tokio::test]
async fn test_spl_token_transfer() {
    let program_test = ProgramTest::new(
        "spl_token",
        token_program_id(),
        processor!(Processor::process),
    );
    let mint = Keypair::new();
    let destination: Keypair = Keypair::new();

    let rent = Rent::default();

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let transaction = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &mint.pubkey(),
                rent.minimum_balance(Mint::LEN),
                Mint::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &mint.pubkey(),
                &payer.pubkey(),
                None,
                6,
            )
            .unwrap(),
            system_instruction::create_account(
                &payer.pubkey(),
                &destination.pubkey(),
                rent.minimum_balance(TokenAccount::LEN),
                TokenAccount::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_account3(
                &token_program_id(),
                &destination.pubkey(),
                &mint.pubkey(),
                &payer.pubkey(),
            )
            .unwrap(),
            spl_token::instruction::mint_to(
                &spl_token::id(),
                &mint.pubkey(),
                &destination.pubkey(),
                &payer.pubkey(),
                &[&payer.pubkey()],
                1_000_000,
            )
            .unwrap(),
        ],
        Some(&payer.pubkey()),
        &[&payer, &mint, &destination],
        recent_blockhash,
    );
    banks_client.process_transaction(transaction).await.unwrap();
}
