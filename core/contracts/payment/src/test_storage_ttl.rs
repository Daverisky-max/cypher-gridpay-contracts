#![cfg(test)]

use crate::*;
use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, Symbol};

fn advance_ledgers(env: &Env, ledgers: u32) {
    env.ledger().with_mut(|l| l.sequence_number += ledgers);
}

#[test]
fn test_set_persistent_extends_ttl() {
    let env = Env::default();
    let contract_id = env.register(PaymentContract, ());
    let key = Symbol::new(&env, "ttl_key");

    env.as_contract(&contract_id, || {
        storage::set_persistent(&env, &key, &42u64);
        assert_eq!(
            env.storage().persistent().get_ttl(&key),
            storage::PERSISTENT_BUMP_AMOUNT
        );
    });
}

#[test]
fn test_get_persistent_extends_ttl() {
    let env = Env::default();
    let contract_id = env.register(PaymentContract, ());
    let key = Symbol::new(&env, "ttl_key");

    env.as_contract(&contract_id, || {
        storage::set_persistent(&env, &key, &42u64);
        storage::extend_instance(&env);
    });

    advance_ledgers(&env, 2 * storage::DAY_IN_LEDGERS);

    env.as_contract(&contract_id, || {
        assert!(env.storage().persistent().get_ttl(&key) < storage::PERSISTENT_LIFETIME_THRESHOLD);
        assert_eq!(storage::get_persistent::<_, u64>(&env, &key), Some(42));
        assert_eq!(
            env.storage().persistent().get_ttl(&key),
            storage::PERSISTENT_BUMP_AMOUNT
        );
    });
}

#[test]
fn test_has_persistent_extends_ttl() {
    let env = Env::default();
    let contract_id = env.register(PaymentContract, ());
    let key = Symbol::new(&env, "ttl_key");

    env.as_contract(&contract_id, || {
        storage::set_persistent(&env, &key, &42u64);
        storage::extend_instance(&env);
    });

    advance_ledgers(&env, 2 * storage::DAY_IN_LEDGERS);

    env.as_contract(&contract_id, || {
        assert!(storage::has_persistent(&env, &key));
        assert_eq!(
            env.storage().persistent().get_ttl(&key),
            storage::PERSISTENT_BUMP_AMOUNT
        );
    });
}

#[test]
fn test_missing_persistent_entry_is_not_extended() {
    let env = Env::default();
    let contract_id = env.register(PaymentContract, ());
    let key = Symbol::new(&env, "missing");

    env.as_contract(&contract_id, || {
        assert_eq!(storage::get_persistent::<_, u64>(&env, &key), None);
        assert!(!storage::has_persistent(&env, &key));
    });
}

#[test]
fn test_extend_instance_extends_ttl() {
    let env = Env::default();
    let contract_id = env.register(PaymentContract, ());

    env.as_contract(&contract_id, || {
        storage::extend_instance(&env);
        assert_eq!(
            env.storage().instance().get_ttl(),
            storage::INSTANCE_BUMP_AMOUNT
        );
    });
}

#[test]
fn test_payment_tag_reads_extend_ttl() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let merchant = Address::generate(&env);
    let customer = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let contract_id = env.register(PaymentContract, ());
    let client = PaymentContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let payment_id = client.create_payment(
        &customer,
        &merchant,
        &1000i128,
        &token,
        &Currency::USDC,
        &3600u64,
        &String::from_str(&env, "ttl payment"),
    );
    let mut tags = Vec::new(&env);
    tags.push_back(BytesN::from_array(&env, &[7u8; 32]));
    client.tag_payment(&merchant, &payment_id, &tags);

    let key = DataKey::Payment(PaymentKey::Tag(payment_id));
    let tag_ttl = || env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));
    let instance_ttl = || env.as_contract(&contract_id, || env.storage().instance().get_ttl());
    assert_eq!(tag_ttl(), storage::PERSISTENT_BUMP_AMOUNT);
    assert_eq!(instance_ttl(), storage::INSTANCE_BUMP_AMOUNT);

    advance_ledgers(&env, 2 * storage::DAY_IN_LEDGERS);
    assert!(tag_ttl() < storage::PERSISTENT_LIFETIME_THRESHOLD);
    assert!(instance_ttl() < storage::INSTANCE_LIFETIME_THRESHOLD);

    assert_eq!(client.get_payment_tags(&payment_id), tags);
    assert_eq!(tag_ttl(), storage::PERSISTENT_BUMP_AMOUNT);

    client.get_payment(&payment_id);
    assert_eq!(instance_ttl(), storage::INSTANCE_BUMP_AMOUNT);
}
