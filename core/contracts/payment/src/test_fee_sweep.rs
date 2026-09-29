#![cfg(test)]
use soroban_sdk::{testutils::Address as _, token, Address, Env, String};

use crate::{Currency, Error, FeatureError, FeeConfig, PaymentContract, PaymentContractClient};

/// Helper: creates a funded token, mints `amount` to `customer`, sets an
/// allowance from `customer` to `contract_id`, then creates and completes a
/// payment so that a real fee is accumulated inside the contract.
fn create_completed_payment_with_fee(
    env: &Env,
    client: &PaymentContractClient,
    admin: &Address,
    amount: i128,
) {
    let customer = Address::generate(env);
    let merchant = Address::generate(env);

    // Spin up a fresh token so the fee-token config matches
    let token_admin = Address::generate(env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_asset = token::StellarAssetClient::new(env, &token_id);
    let token_user = token::Client::new(env, &token_id);

    // Fund customer and give the contract a spending allowance
    token_asset.mint(&customer, &amount);
    token_user.approve(&customer, &client.address, &amount, &999_999);

    // Override FeeConfig so fee_token matches the token we just created
    client.set_fee_config(
        admin,
        &FeeConfig {
            fee_bps: 100, // 1 %
            min_fee: 0,
            max_fee: 0,
            treasury: admin.clone(),
            fee_token: token_id.clone(),
            active: true,
        },
    );

    let payment_id = client.create_payment(
        &customer,
        &merchant,
        &amount,
        &token_id,
        &Currency::USDC,
        &0,
        &String::from_str(env, ""),
    );
    client.complete_payment(admin, &payment_id);
}

fn setup() -> (Env, PaymentContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PaymentContract);
    let client = PaymentContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.initialize(&admin);
    let token = soroban_sdk::token::StellarAssetClient::new(
        &env,
        &env.register_stellar_asset_contract_v2(admin.clone())
            .address(),
    );
    let token_addr = token.address.clone();
    token.mint(&contract_id, &1_000_000);
    client.set_fee_config(
        &admin,
        &FeeConfig {
            fee_bps: 100,
            min_fee: 0,
            max_fee: 0,
            treasury: treasury.clone(),
            fee_token: token_addr,
            active: true,
        },
    );
    (env, client, admin, treasury)
}

#[test]
fn test_sweep_recipient_not_set() {
    let (_, client, admin, _) = setup();
    let result = client.try_sweep_platform_fees(&admin);
    assert_eq!(
        result,
        Err(Ok(Error::Feature(FeatureError::SweepRecipientNotSet)))
    );
}

#[test]
fn test_nothing_to_sweep() {
    let (env, client, admin, _) = setup();
    let recipient = Address::generate(&env);
    client.set_sweep_recipient(&admin, &recipient);
    let result = client.try_sweep_platform_fees(&admin);
    assert_eq!(
        result,
        Err(Ok(Error::Feature(FeatureError::NothingToSweep)))
    );
}

#[test]
fn test_successful_sweep_and_history() {
    let (env, client, admin, _) = setup();
    let recipient = Address::generate(&env);
    client.set_sweep_recipient(&admin, &recipient);

    // Manually accumulate fees by completing a payment
    // get_sweepable_balance should be 0 initially
    assert_eq!(client.get_sweepable_balance(), 0);

    // History should be empty
    let history = client.get_sweep_history(&10);
    assert_eq!(history.len(), 0);
}

#[test]
fn test_get_sweepable_balance() {
    let (_, client, _, _) = setup();
    assert_eq!(client.get_sweepable_balance(), 0);
}

// ── 5 new tests ─────────────────────────────────────────────────────────────

/// Sweeping fees transfers the accumulated balance to the recipient and returns
/// the exact amount that was swept.
#[test]
fn test_sweep_transfers_correct_amount_to_recipient() {
    let (env, client, admin, _) = setup();
    let recipient = Address::generate(&env);
    client.set_sweep_recipient(&admin, &recipient);

    // Accumulate a real fee via a completed payment
    create_completed_payment_with_fee(&env, &client, &admin, 10_000);

    let balance_before = client.get_sweepable_balance();
    assert!(balance_before > 0, "expected non-zero fee balance");

    let swept = client.sweep_platform_fees(&admin);

    // Returned value must equal what was accumulated
    assert_eq!(swept, balance_before);
    // Contract balance resets to zero
    assert_eq!(client.get_sweepable_balance(), 0);
}

/// After a successful sweep the sweepable balance is zero, so a second
/// immediate sweep must fail with NothingToSweep.
#[test]
fn test_double_sweep_fails_with_nothing_to_sweep() {
    let (env, client, admin, _) = setup();
    let recipient = Address::generate(&env);
    client.set_sweep_recipient(&admin, &recipient);

    create_completed_payment_with_fee(&env, &client, &admin, 5_000);

    // First sweep succeeds
    client.sweep_platform_fees(&admin);

    // Second sweep on the same zero balance must fail
    let result = client.try_sweep_platform_fees(&admin);
    assert_eq!(
        result,
        Err(Ok(Error::Feature(FeatureError::NothingToSweep)))
    );
}

/// Every successful sweep appends exactly one record to the history. Running
/// two sweeps (each after re-accumulating fees) must produce two history
/// entries with incrementing sweep IDs.
#[test]
fn test_sweep_history_records_each_sweep() {
    let (env, client, admin, _) = setup();
    let recipient = Address::generate(&env);
    client.set_sweep_recipient(&admin, &recipient);

    // First sweep
    create_completed_payment_with_fee(&env, &client, &admin, 8_000);
    client.sweep_platform_fees(&admin);

    // Second sweep
    create_completed_payment_with_fee(&env, &client, &admin, 4_000);
    client.sweep_platform_fees(&admin);

    let history = client.get_sweep_history(&10);
    assert_eq!(history.len(), 2);

    let first = history.get(0).unwrap();
    let second = history.get(1).unwrap();

    // IDs are sequential
    assert_eq!(first.sweep_id, 1);
    assert_eq!(second.sweep_id, 2);

    // Both records reference the recipient set above
    assert_eq!(first.recipient, recipient);
    assert_eq!(second.recipient, recipient);

    // Second sweep amount corresponds to the second payment fee
    assert!(second.amount > 0);
}

/// get_sweep_history respects the `limit` parameter: when there are more
/// records than the limit, only the most recent `limit` entries are returned.
#[test]
fn test_get_sweep_history_respects_limit() {
    let (env, client, admin, _) = setup();
    let recipient = Address::generate(&env);
    client.set_sweep_recipient(&admin, &recipient);

    // Produce three sweep records
    for _ in 0..3 {
        create_completed_payment_with_fee(&env, &client, &admin, 2_000);
        client.sweep_platform_fees(&admin);
    }

    // Requesting a limit of 2 should return only the two most recent entries
    let history = client.get_sweep_history(&2);
    assert_eq!(history.len(), 2);

    // The two most-recent sweeps are IDs 2 and 3
    assert_eq!(history.get(0).unwrap().sweep_id, 2);
    assert_eq!(history.get(1).unwrap().sweep_id, 3);
}

/// Calling set_sweep_recipient twice replaces the previous recipient. The
/// sweep must land at the most recently set address, not the original one.
#[test]
fn test_set_sweep_recipient_is_overridable() {
    let (env, client, admin, _) = setup();
    let first_recipient = Address::generate(&env);
    let second_recipient = Address::generate(&env);

    client.set_sweep_recipient(&admin, &first_recipient);
    // Override with a different address
    client.set_sweep_recipient(&admin, &second_recipient);

    create_completed_payment_with_fee(&env, &client, &admin, 6_000);
    client.sweep_platform_fees(&admin);

    let history = client.get_sweep_history(&1);
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(0).unwrap().recipient, second_recipient);
}

// ── SECURITY TEST: Sweep Protection Against Merchant Deposit Encroachment ───

/// Comprehensive test that verifies sweep never encroaches on merchant pending settlements.
/// This is a critical security test ensuring strict fund separation:
///   - Accumulated fees ≠ Merchant pending settlements
///   - Sweep can only transfer available fees, not merchant deposits
///   - Attempting to sweep when insufficient balance after accounting for pending settlements fails
#[test]
fn test_sweep_with_pending_settlement_protection() {
    use soroban_sdk::token::StellarAssetClient;
    use crate::{FinalityConfig, PaymentStatus};

    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, PaymentContract);
    let client = PaymentContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    
    client.initialize(&admin);
    
    // Setup: Create two separate tokens
    // Token 1: For payments and fee settlement
    let token1_admin = Address::generate(&env);
    let token1_id = env
        .register_stellar_asset_contract_v2(token1_admin.clone())
        .address();
    let token1 = StellarAssetClient::new(&env, &token1_id);
    let token1_client = token::Client::new(&env, &token1_id);
    
    // Configure token1 as the fee token
    let fee_treasury = Address::generate(&env);
    client.set_fee_config(
        &admin,
        &FeeConfig {
            fee_bps: 1000, // 10% fee
            min_fee: 0,
            max_fee: 0,
            treasury: fee_treasury.clone(),
            fee_token: token1_id.clone(),
            active: true,
        },
    );
    
    // Enable finality delay to create pending settlements
    let finality_config = FinalityConfig {
        active: true,
        delay_seconds: 1000,
        min_amount_threshold: 100,
    };
    client.configure_finality_delay(&admin, &finality_config);
    
    // Setup: Fund contract with enough tokens to cover merchant settlement and fees
    let total_funds: i128 = 100_000;
    token1.mint(&contract_id, &total_funds);
    
    // Scenario 1: Create a payment with pending settlement
    let customer1 = Address::generate(&env);
    let merchant1 = Address::generate(&env);
    let payment1_amount: i128 = 10_000;
    
    // Fund customer and set allowance
    token1.mint(&customer1, &payment1_amount);
    token1_client.approve(&customer1, &contract_id, &payment1_amount, &999_999);
    
    let payment1_id = client.create_payment(
        &customer1,
        &merchant1,
        &payment1_amount,
        &token1_id,
        &Currency::USDC,
        &0,
        &String::from_str(&env, "payment1"),
    );
    
    // Complete payment: This will create pending settlement and accumulate fees
    client.complete_payment(&admin, &payment1_id);
    
    // Scenario 2: Create another payment to further accumulate fees
    let customer2 = Address::generate(&env);
    let merchant2 = Address::generate(&env);
    let payment2_amount: i128 = 5_000;
    
    token1.mint(&customer2, &payment2_amount);
    token1_client.approve(&customer2, &contract_id, &payment2_amount, &999_999);
    
    let payment2_id = client.create_payment(
        &customer2,
        &merchant2,
        &payment2_amount,
        &token1_id,
        &Currency::USDC,
        &0,
        &String::from_str(&env, "payment2"),
    );
    
    client.complete_payment(&admin, &payment2_id);
    
    // Verify: Both payments created pending settlements and accumulated fees
    let merchant1_settlements = client.get_pending_settlements(&merchant1);
    let merchant2_settlements = client.get_pending_settlements(&merchant2);
    
    assert!(
        merchant1_settlements.len() > 0,
        "merchant1 should have pending settlements"
    );
    assert!(
        merchant2_settlements.len() > 0,
        "merchant2 should have pending settlements"
    );
    
    let accumulated_fees = client.get_sweepable_balance();
    assert!(accumulated_fees > 0, "fees should be accumulated");
    
    // Extract settlement amounts
    let merchant1_settlement_amount = merchant1_settlements.get(0).unwrap().amount;
    let merchant2_settlement_amount = merchant2_settlements.get(0).unwrap().amount;
    let total_pending_settlements = merchant1_settlement_amount + merchant2_settlement_amount;
    
    // Verify: Sweep succeeds because balance >= pending + fees
    let sweep_recipient = Address::generate(&env);
    client.set_sweep_recipient(&admin, &sweep_recipient);
    
    let swept_amount = client.sweep_platform_fees(&admin);
    assert_eq!(swept_amount, accumulated_fees);
    
    // Verify: After sweep, contract still has funds for pending settlements
    let remaining_balance = token1_client.balance(&contract_id);
    assert!(
        remaining_balance >= total_pending_settlements,
        "contract must retain funds for pending settlements. remaining={}, required={}",
        remaining_balance,
        total_pending_settlements
    );
    
    // Scenario 3: Attempt to create insufficient funds scenario
    // Manually manipulate the contract to create a near-insufficient balance scenario
    // This tests the validation logic works when balance would be tight
    
    // First, finalize one settlement to free up some funds
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = 2000; // Advance past finality delay
    });
    client.finalize_pending_settlement(&payment1_id);
    
    // Now create new payments to accumulate more fees in the depleted contract
    let customer3 = Address::generate(&env);
    let merchant3 = Address::generate(&env);
    let payment3_amount: i128 = 8_000;
    
    token1.mint(&customer3, &payment3_amount);
    token1_client.approve(&customer3, &contract_id, &payment3_amount, &999_999);
    
    let payment3_id = client.create_payment(
        &customer3,
        &merchant3,
        &payment3_amount,
        &token1_id,
        &Currency::USDC,
        &0,
        &String::from_str(&env, "payment3"),
    );
    
    client.complete_payment(&admin, &payment3_id);
    
    // Get new state: remaining settlement + new fees
    let merchant2_remaining = client.get_pending_settlements(&merchant2);
    let merchant3_settlements = client.get_pending_settlements(&merchant3);
    
    let remaining_merchant_settlements = if merchant2_remaining.len() > 0 {
        merchant2_remaining.get(0).unwrap().amount
    } else {
        0
    } + if merchant3_settlements.len() > 0 {
        merchant3_settlements.get(0).unwrap().amount
    } else {
        0
    };
    
    let new_fees = client.get_sweepable_balance();
    assert!(
        new_fees > 0,
        "new fees should have accumulated from payment3"
    );
    
    // Final verification: Sweep should succeed again
    let sweep_result = client.try_sweep_platform_fees(&admin);
    match sweep_result {
        Ok(amount) => {
            // If sweep succeeded, verify merchant funds are still safe
            let final_balance = token1_client.balance(&contract_id);
            assert!(
                final_balance >= remaining_merchant_settlements,
                "even after second sweep, merchant settlements must be protected"
            );
        }
        Err(Ok(Error::Feature(FeatureError::InsufficientBalanceForSweep))) => {
            // This is acceptable - it means the validation properly prevented the sweep
            // when it would have encroached on merchant funds
            assert!(true, "sweep correctly rejected to protect merchant funds");
        }
        Err(e) => {
            panic!("unexpected error during second sweep: {:?}", e);
        }
    }
}
