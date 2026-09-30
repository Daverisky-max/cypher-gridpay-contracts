#![cfg(test)]

use crate::*;
use soroban_sdk::{testutils::Address as _, Address, Bytes, Env};

fn setup() -> (Env, EscrowContractClient<'static>, Address) {
    let env = Env::default();
    let id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    env.mock_all_auths();
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn test_direct_update_required_signatures_rejected() {
    let (env, client, admin) = setup();
    let _ = env;
    let result = client.try_update_required_signatures(&admin, &1u32);
    assert_eq!(result, Err(Ok(Error::Basic(BasicError::Unauthorized))));
}

#[test]
fn test_threshold_change_via_multisig() {
    let (env, client, admin) = setup();

    let admin2 = Address::generate(&env);
    client.add_admin(&admin, &admin2);

    // With threshold=1 (default), one approval from proposer is enough to execute.
    let proposal_id = client.propose_action(
        &admin,
        &ActionType::UpdateThreshold(2u32),
        &admin2,
        &Bytes::new(&env),
    );
    client.execute_action(&proposal_id);

    let config = client.get_multisig_config();
    assert_eq!(config.required_signatures, 2);
}

#[test]
fn test_threshold_zero_rejected() {
    let (env, client, admin) = setup();

    let proposal_id = client.propose_action(
        &admin,
        &ActionType::UpdateThreshold(0u32),
        &admin,
        &Bytes::new(&env),
    );

    let result = client.try_execute_action(&proposal_id);
    assert_eq!(result, Err(Ok(Error::Escrow(EscrowError::InvalidThreshold))));
}

#[test]
fn test_threshold_exceeds_admin_count_rejected() {
    let (env, client, admin) = setup();

    // Only 1 admin exists; requesting threshold=2 should fail.
    let proposal_id = client.propose_action(
        &admin,
        &ActionType::UpdateThreshold(2u32),
        &admin,
        &Bytes::new(&env),
    );

    let result = client.try_execute_action(&proposal_id);
    assert_eq!(
        result,
        Err(Ok(Error::Basic(BasicError::InsufficientAdmins)))
    );
}

#[test]
fn test_threshold_increase_invalidates_pending_proposal_approvals() {
    let (env, client, admin) = setup();

    let admin2 = Address::generate(&env);
    client.add_admin(&admin, &admin2);

    // A pending action that collected its approvals under the old threshold.
    let pending = client.propose_action(
        &admin,
        &ActionType::UpdateThreshold(1u32),
        &admin,
        &Bytes::new(&env),
    );

    // Raise the required signatures to 2 through a separate proposal.
    let raise = client.propose_action(
        &admin,
        &ActionType::UpdateThreshold(2u32),
        &admin,
        &Bytes::new(&env),
    );
    client.execute_action(&raise);
    assert_eq!(client.get_multisig_config().required_signatures, 2);

    // The stale proposal cannot ride on approvals gathered under the old rule.
    let stale = client.try_execute_action(&pending);
    assert_eq!(
        stale,
        Err(Ok(Error::Escrow(EscrowError::InvalidStatus)))
    );

    // Re-approving restarts collection under the new threshold: one admin is
    // still not enough, two are.
    client.approve_action(&admin, &pending);
    let still_short = client.try_execute_action(&pending);
    assert_eq!(
        still_short,
        Err(Ok(Error::Escrow(EscrowError::InvalidStatus)))
    );

    client.approve_action(&admin2, &pending);
    client.execute_action(&pending);
    assert_eq!(client.get_multisig_config().required_signatures, 1);
}
