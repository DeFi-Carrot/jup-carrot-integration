use amm::state::Vault;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[tokio::test]
async fn test_load_vault_account() {
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
    let _vault = Vault::load(&base64_data).unwrap();
}
