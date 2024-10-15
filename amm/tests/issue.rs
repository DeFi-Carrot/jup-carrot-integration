use amm::constants::{CARROT_LOG_PROGRAM, CARROT_PROGRAM};
use solana_program_test::ProgramTest;
use solana_sdk::rent::Rent;

#[tokio::test]
async fn test_issue() {
    let mut program_test = ProgramTest::default();
    program_test.prefer_bpf(true);
    program_test.add_program("carrot", CARROT_PROGRAM, None);
    program_test.add_program("carrot-log", CARROT_LOG_PROGRAM, None);

    let rent = Rent::default();

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
}
