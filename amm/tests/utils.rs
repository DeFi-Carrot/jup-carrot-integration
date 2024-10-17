use base64::{engine::general_purpose::STANDARD, Engine};
use solana_program_test::BanksClient;
use std::collections::HashMap;
use std::str::FromStr;

use jupiter_amm_interface::AccountMap;
use serde_json::Value;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn load_account_map_from_file() -> AccountMap {
    let paths = [
        "tests/fixtures/vault.json",
        "tests/fixtures/shares_mint.json",
        "tests/fixtures/usdc_mint.json",
        "tests/fixtures/usdt_mint.json",
        "tests/fixtures/pyusd_mint.json",
        "tests/fixtures/vault_usdc_ata.json",
        "tests/fixtures/vault_usdt_ata.json",
        "tests/fixtures/vault_pyusd_ata.json",
        "tests/fixtures/usdc_pyth_oracle.json",
        "tests/fixtures/usdt_pyth_oracle.json",
        "tests/fixtures/pyusd_pyth_oracle.json",
    ];
    let mut map = HashMap::new();
    for path in paths.iter() {
        let (address, account) = load_account_from_file(path);
        map.insert(address, account);
    }

    map
}

pub async fn load_account_map_from_bank(
    banks_client: &mut BanksClient,
    accounts: &[Pubkey],
) -> AccountMap {
    let mut map = HashMap::new();
    for addr in accounts {
        let account = banks_client
            .get_account(addr.clone())
            .await
            .unwrap()
            .unwrap();
        map.insert(addr.clone(), account);
    }

    map
}

fn load_account_from_file(path: &str) -> (Pubkey, Account) {
    // read json file representing account
    let path = Path::new(path);
    let mut file = File::open(&path).unwrap();
    let mut data = String::new();
    file.read_to_string(&mut data).unwrap();

    let account_data_json: Value = serde_json::from_str(&data).unwrap();

    let pubkey_str = account_data_json["pubkey"].as_str().unwrap();
    let pubkey = Pubkey::from_str(pubkey_str).unwrap();

    let base64_data = STANDARD
        .decode(account_data_json["account"]["data"][0].as_str().unwrap())
        .unwrap();

    let account = Account {
        lamports: account_data_json["account"]["lamports"].as_u64().unwrap(),
        owner: Pubkey::from_str(account_data_json["account"]["owner"].as_str().unwrap()).unwrap(),
        data: base64_data,
        executable: account_data_json["account"]["executable"]
            .as_bool()
            .unwrap(),
        rent_epoch: account_data_json["account"]["rentEpoch"].as_u64().unwrap(),
    };

    (pubkey, account)
}
