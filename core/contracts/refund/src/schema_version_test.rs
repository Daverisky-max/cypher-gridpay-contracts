#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

#[test]
fn test_schema_version_initialized_to_one() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(RefundContract, ());
    let client = RefundContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(client.get_schema_version(), 1);
}

#[test]
fn test_migrate_schema_rejects_already_at_target() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(RefundContract, ());
    let client = RefundContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    client.migrate_schema(&admin, &2);
    assert_eq!(client.get_schema_version(), 2);

    let result = client.try_migrate_schema(&admin, &2);
    assert_eq!(result, Err(Ok(Error::Ext(ExtError::SchemaAlreadyAtTarget))));
}

#[test]
fn test_dry_run_migrate_schema_is_idempotent() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(RefundContract, ());
    let client = RefundContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Two consecutive dry runs must produce identical reports.
    let first = client.dry_run_migrate_schema(&2);
    let second = client.dry_run_migrate_schema(&2);

    assert_eq!(first, second);
    assert_eq!(first.current_version, 1);
    assert_eq!(first.target_version, 2);
    assert_eq!(first.converted_records, 0);
    assert!(first.dry_run);
    assert!(first.gas_estimate > 0);

    // The dry run must not mutate storage: schema version stays put and a
    // subsequent real migration still succeeds.
    assert_eq!(client.get_schema_version(), 1);
    client.migrate_schema(&admin, &2);
    assert_eq!(client.get_schema_version(), 2);
}
