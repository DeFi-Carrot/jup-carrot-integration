use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::HashMap;
use std::str::FromStr;

use jupiter_amm_interface::AccountMap;
use serde_json::Value;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn init_account_map() -> AccountMap {
    let paths = [
        "tests/fixtures/vault.json",
        "tests/fixtures/shares.json",
        "tests/fixtures/vault_usdc_ata.json",
        "tests/fixtures/vault_usdt_ata.json",
        "tests/fixtures/vault_pyusd_ata.json",
    ];
    let mut map = HashMap::new();
    for path in paths.iter() {
        let (address, account) = load_account(path);
        map.insert(address, account);
    }

    map
}

pub fn load_account(path: &str) -> (Pubkey, Account) {
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
