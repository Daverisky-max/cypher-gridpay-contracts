#![no_std]
use escrow::EscrowContractClient;
use payments::PaymentContractClient;
use refund::RefundContractClient;
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, Env, String,
};

#[contracterror]
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    RotationNotPending = 4,
    RotationTimelockActive = 5,
    RotationAlreadyPending = 6,
}

/// Identifies which downstream contract address is being rotated.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum ContractTarget {
    Payment,
    Escrow,
    Refund,
}

/// A contract address rotation that has been proposed but has not yet taken
/// effect because the timelock has not elapsed.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PendingRotation {
    /// The new contract address that will take effect after the timelock.
    pub new_address: Address,
    /// The admin who proposed the rotation.
    pub proposed_by: Address,
    /// Ledger timestamp (in seconds) at which the rotation was proposed.
    pub proposed_at: u64,
}

/// Timelock duration for contract address rotations (Issue #77).
///
/// A proposed rotation of the payment, escrow, or refund contract address only
/// takes effect after this many seconds have elapsed, giving operators time to
/// detect and react to a malicious or mistaken proposal.
pub const CONTRACT_ROTATION_TIMELOCK: u64 = 48 * 60 * 60;

/// Unified health report for the payment, escrow, and refund contracts
/// (Issue #80).
///
/// Returned by [`AdminContract::get_system_status`] so frontends and monitoring
/// systems can check the operational status of every core contract in a single
/// RPC query.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SystemStatus {
    /// Whether the payment contract is globally paused.
    pub payment_paused: bool,
    /// Storage schema version reported by the payment contract.
    pub payment_schema_version: u32,
    /// Whether the escrow contract is globally paused.
    pub escrow_paused: bool,
    /// Storage schema version reported by the escrow contract.
    pub escrow_schema_version: u32,
    /// Whether the refund contract is globally paused.
    pub refund_paused: bool,
    /// Storage schema version reported by the refund contract.
    pub refund_schema_version: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractTargetUpdated {
    /// Which contract target was rotated.
    pub target: ContractTarget,
    /// The contract address in effect before the rotation.
    pub old_address: Address,
    /// The contract address now in effect.
    pub new_address: Address,
    /// Ledger timestamp (in seconds) at which the rotation took effect.
    pub updated_at: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Pauser,
    PaymentContract,
    EscrowContract,
    RefundContract,
    PendingPaymentRotation,
    PendingEscrowRotation,
    PendingRefundRotation,
}

#[contract]
pub struct AdminContract;

#[contractimpl]
impl AdminContract {
    /// Initializes the admin contract with the addresses of the payment, escrow,
    /// and refund contracts.
    ///
    /// # Parameters
    /// - `admin`: the address authorized to manage the contract.
    /// - `pauser`: the address authorized to pause/unpause the platform.
    /// - `payment_contract`: the deployed payment contract address.
    /// - `escrow_contract`: the deployed escrow contract address.
    /// - `refund_contract`: the deployed refund contract address.
    ///
    /// # Returns
    /// Returns `Ok(())` when initialization succeeds.
    ///
    /// # Errors
    /// Returns `Error::AlreadyInitialized` if the contract has already been set up.
    pub fn initialize(
        env: Env,
        admin: Address,
        pauser: Address,
        payment_contract: Address,
        escrow_contract: Address,
        refund_contract: Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Pauser, &pauser);
        env.storage()
            .instance()
            .set(&DataKey::PaymentContract, &payment_contract);
        env.storage()
            .instance()
            .set(&DataKey::EscrowContract, &escrow_contract);
        env.storage()
            .instance()
            .set(&DataKey::RefundContract, &refund_contract);

        Ok(())
    }

    /// Pauses the payment, escrow, and refund contracts in one Soroban call.
    ///
    /// # Parameters
    /// - `pauser`: the pauser address that must be authorized.
    /// - `reason`: a human-readable explanation for the emergency pause.
    ///
    /// # Returns
    /// Returns `Ok(())` when all child contracts are paused successfully.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// and `Error::Unauthorized` if the provided pauser address does not match the
    /// stored pauser.
    pub fn emergency_pause_all(env: Env, pauser: Address, reason: String) -> Result<(), Error> {
        pauser.require_auth();

        let stored_pauser: Address = env
            .storage()
            .instance()
            .get(&DataKey::Pauser)
            .ok_or(Error::NotInitialized)?;
        if pauser != stored_pauser {
            return Err(Error::Unauthorized);
        }

        let payment_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentContract)
            .ok_or(Error::NotInitialized)?;
        let escrow_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::EscrowContract)
            .ok_or(Error::NotInitialized)?;
        let refund_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::RefundContract)
            .ok_or(Error::NotInitialized)?;

        PaymentContractClient::new(&env, &payment_contract).pause_contract(&pauser, &reason);
        EscrowContractClient::new(&env, &escrow_contract).pause_contract(&pauser, &reason);
        RefundContractClient::new(&env, &refund_contract).pause_contract(&pauser, &reason);

        Ok(())
    }

    /// Unpauses the payment, escrow, and refund contracts in one Soroban call.
    ///
    /// # Parameters
    /// - `pauser`: the pauser address that must be authorized.
    ///
    /// # Returns
    /// Returns `Ok(())` when all child contracts are unpaused successfully.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// and `Error::Unauthorized` if the provided pauser address does not match the
    /// stored pauser.
    pub fn emergency_unpause_all(env: Env, pauser: Address) -> Result<(), Error> {
        pauser.require_auth();

        let stored_pauser: Address = env
            .storage()
            .instance()
            .get(&DataKey::Pauser)
            .ok_or(Error::NotInitialized)?;
        if pauser != stored_pauser {
            return Err(Error::Unauthorized);
        }

        let payment_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentContract)
            .ok_or(Error::NotInitialized)?;
        let escrow_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::EscrowContract)
            .ok_or(Error::NotInitialized)?;
        let refund_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::RefundContract)
            .ok_or(Error::NotInitialized)?;

        PaymentContractClient::new(&env, &payment_contract).unpause_contract(&pauser);
        EscrowContractClient::new(&env, &escrow_contract).unpause_contract(&pauser);
        RefundContractClient::new(&env, &refund_contract).unpause_contract(&pauser);

        Ok(())
    }

    /// Proposes a rotation of the stored payment contract address (Issue #77).
    ///
    /// The new address does not take effect immediately. It is stored as a
    /// pending rotation and only becomes active once
    /// [`AdminContract::execute_payment_rotation`] is called after the
    /// 48-hour timelock has elapsed. This prevents a single compromised admin
    /// from immediately redirecting the orchestrator to a malicious contract.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `payment_contract`: the new payment contract address.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin, and `Error::RotationAlreadyPending` if a rotation for this
    /// target is already pending.
    pub fn set_payment_contract(
        env: Env,
        admin: Address,
        payment_contract: Address,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;
        Self::propose_rotation(
            &env,
            DataKey::PendingPaymentRotation,
            &admin,
            &payment_contract,
        )
    }

    /// Proposes a rotation of the stored escrow contract address (Issue #77).
    ///
    /// The new address does not take effect immediately. It is stored as a
    /// pending rotation and only becomes active once
    /// [`AdminContract::execute_escrow_rotation`] is called after the
    /// 48-hour timelock has elapsed.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `escrow_contract`: the new escrow contract address.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin, and `Error::RotationAlreadyPending` if a rotation for this
    /// target is already pending.
    pub fn set_escrow_contract(
        env: Env,
        admin: Address,
        escrow_contract: Address,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;
        Self::propose_rotation(
            &env,
            DataKey::PendingEscrowRotation,
            &admin,
            &escrow_contract,
        )
    }

    /// Proposes a rotation of the stored refund contract address (Issue #77).
    ///
    /// The new address does not take effect immediately. It is stored as a
    /// pending rotation and only becomes active once
    /// [`AdminContract::execute_refund_rotation`] is called after the
    /// 48-hour timelock has elapsed.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `refund_contract`: the new refund contract address.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin, and `Error::RotationAlreadyPending` if a rotation for this
    /// target is already pending.
    pub fn set_refund_contract(
        env: Env,
        admin: Address,
        refund_contract: Address,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;
        Self::propose_rotation(
            &env,
            DataKey::PendingRefundRotation,
            &admin,
            &refund_contract,
        )
    }

    /// Executes a pending payment contract address rotation (Issue #77).
    ///
    /// The rotation only takes effect if at least
    /// [`CONTRACT_ROTATION_TIMELOCK`] seconds have elapsed since it was
    /// proposed. On success the stored payment contract address is updated,
    /// the pending rotation is cleared, and a [`ContractTargetUpdated`] event
    /// is emitted.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin, `Error::RotationNotPending` if no rotation is pending for
    /// this target, and `Error::RotationTimelockActive` if the 48-hour timelock
    /// has not yet elapsed.
    pub fn execute_payment_rotation(env: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;
        Self::execute_rotation(
            &env,
            ContractTarget::Payment,
            DataKey::PendingPaymentRotation,
            DataKey::PaymentContract,
        )
    }

    /// Executes a pending escrow contract address rotation (Issue #77).
    ///
    /// The rotation only takes effect if at least
    /// [`CONTRACT_ROTATION_TIMELOCK`] seconds have elapsed since it was
    /// proposed. On success the stored escrow contract address is updated,
    /// the pending rotation is cleared, and a [`ContractTargetUpdated`] event
    /// is emitted.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin, `Error::RotationNotPending` if no rotation is pending for
    /// this target, and `Error::RotationTimelockActive` if the 48-hour timelock
    /// has not yet elapsed.
    pub fn execute_escrow_rotation(env: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;
        Self::execute_rotation(
            &env,
            ContractTarget::Escrow,
            DataKey::PendingEscrowRotation,
            DataKey::EscrowContract,
        )
    }

    /// Executes a pending refund contract address rotation (Issue #77).
    ///
    /// The rotation only takes effect if at least
    /// [`CONTRACT_ROTATION_TIMELOCK`] seconds have elapsed since it was
    /// proposed. On success the stored refund contract address is updated,
    /// the pending rotation is cleared, and a [`ContractTargetUpdated`] event
    /// is emitted.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin, `Error::RotationNotPending` if no rotation is pending for
    /// this target, and `Error::RotationTimelockActive` if the 48-hour timelock
    /// has not yet elapsed.
    pub fn execute_refund_rotation(env: Env, admin: Address) -> Result<(), Error> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;
        Self::execute_rotation(
            &env,
            ContractTarget::Refund,
            DataKey::PendingRefundRotation,
            DataKey::RefundContract,
        )
    }

    /// Cancels a pending contract address rotation (Issue #77).
    ///
    /// Allows the admin to abort a rotation during the timelock window, for
    /// example if the proposed address was found to be malicious or mistaken.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `target`: the contract target whose pending rotation should be canceled.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin, and `Error::RotationNotPending` if no rotation is pending
    /// for this target.
    pub fn cancel_proposed_rotation(
        env: Env,
        admin: Address,
        target: ContractTarget,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;
        let pending_key = Self::target_key(&target);
        if !env.storage().instance().has(&pending_key) {
            return Err(Error::RotationNotPending);
        }
        env.storage().instance().remove(&pending_key);
        Ok(())
    }

    /// Returns the pending rotation for a contract target, if one exists
    /// (Issue #77).
    ///
    /// # Parameters
    /// - `target`: the contract target to query.
    ///
    /// # Returns
    /// The pending rotation, or `None` if no rotation is pending for the target.
    pub fn get_pending_rotation(env: Env, target: ContractTarget) -> Option<PendingRotation> {
        let pending_key = Self::target_key(&target);
        env.storage().instance().get(&pending_key)
    }

    /// Returns a unified health report for the payment, escrow, and refund
    /// contracts (Issue #80).
    ///
    /// Queries the pause status and storage schema version of each core
    /// contract in a single call, so frontends and monitoring systems can
    /// check the operational status of the whole platform with one RPC query.
    ///
    /// # Parameters
    /// - `env` - The Soroban environment.
    ///
    /// # Returns
    /// A [`SystemStatus`] with the pause flag and schema version of each
    /// core contract.
    ///
    /// # Panics
    /// Panics if the admin contract has not been initialized.
    pub fn get_system_status(env: Env) -> SystemStatus {
        let payment_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentContract)
            .unwrap_or_else(|| panic!("admin contract not initialized"));
        let escrow_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::EscrowContract)
            .unwrap_or_else(|| panic!("admin contract not initialized"));
        let refund_contract: Address = env
            .storage()
            .instance()
            .get(&DataKey::RefundContract)
            .unwrap_or_else(|| panic!("admin contract not initialized"));

        let payment = PaymentContractClient::new(&env, &payment_contract);
        let escrow = EscrowContractClient::new(&env, &escrow_contract);
        let refund = RefundContractClient::new(&env, &refund_contract);

        SystemStatus {
            payment_paused: payment.get_pause_state().globally_paused,
            payment_schema_version: payment.get_schema_version(),
            escrow_paused: escrow.get_pause_state().globally_paused,
            escrow_schema_version: escrow.get_schema_version(),
            refund_paused: refund.get_pause_state().globally_paused,
            refund_schema_version: refund.get_schema_version(),
        }
    }

    /// Maps a contract target to its pending-rotation storage key.
    fn target_key(target: &ContractTarget) -> DataKey {
        match target {
            ContractTarget::Payment => DataKey::PendingPaymentRotation,
            ContractTarget::Escrow => DataKey::PendingEscrowRotation,
            ContractTarget::Refund => DataKey::PendingRefundRotation,
        }
    }

    /// Verifies the caller is the stored admin.
    fn require_admin(env: &Env, admin: &Address) -> Result<(), Error> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if *admin != stored_admin {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    /// Stores a new pending rotation, rejecting duplicate proposals.
    fn propose_rotation(
        env: &Env,
        pending_key: DataKey,
        admin: &Address,
        new_address: &Address,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&pending_key) {
            return Err(Error::RotationAlreadyPending);
        }
        let rotation = PendingRotation {
            new_address: new_address.clone(),
            proposed_by: admin.clone(),
            proposed_at: env.ledger().timestamp(),
        };
        env.storage().instance().set(&pending_key, &rotation);
        Ok(())
    }

    /// Applies a pending rotation once its timelock has elapsed.
    fn execute_rotation(
        env: &Env,
        target: ContractTarget,
        pending_key: DataKey,
        stored_key: DataKey,
    ) -> Result<(), Error> {
        let rotation: PendingRotation = env
            .storage()
            .instance()
            .get(&pending_key)
            .ok_or(Error::RotationNotPending)?;

        let now = env.ledger().timestamp();
        let eligible_at = rotation.proposed_at + CONTRACT_ROTATION_TIMELOCK;
        if now < eligible_at {
            return Err(Error::RotationTimelockActive);
        }

        let old_address: Address = env
            .storage()
            .instance()
            .get(&stored_key)
            .ok_or(Error::NotInitialized)?;

        env.storage()
            .instance()
            .set(&stored_key, &rotation.new_address);
        env.storage().instance().remove(&pending_key);

        (ContractTargetUpdated {
            target,
            old_address,
            new_address: rotation.new_address,
            updated_at: now,
        })
        .publish(env);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events, Ledger};
    use soroban_sdk::{Symbol, TryFromVal};

    fn setup_payment(env: &Env, admin: &Address) -> Address {
        let contract_id = env.register(payments::PaymentContract, ());
        let client = PaymentContractClient::new(env, &contract_id);
        client.initialize(admin);
        contract_id
    }

    fn setup_escrow(env: &Env, admin: &Address) -> Address {
        let contract_id = env.register(escrow::EscrowContract, ());
        let client = EscrowContractClient::new(env, &contract_id);
        client.initialize(admin);
        contract_id
    }

    fn setup_refund(env: &Env, admin: &Address) -> Address {
        let contract_id = env.register(refund::RefundContract, ());
        let client = RefundContractClient::new(env, &contract_id);
        client.initialize(admin);
        contract_id
    }

    #[test]
    fn test_get_system_status() {
        let env = Env::default();
        env.mock_all_auths();

        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(&env, &admin_contract_id);

        let admin = Address::generate(&env);
        let pauser = Address::generate(&env);
        let payment_contract = setup_payment(&env, &pauser);
        let escrow_contract = setup_escrow(&env, &pauser);
        let refund_contract = setup_refund(&env, &pauser);

        client.initialize(
            &admin,
            &pauser,
            &payment_contract,
            &escrow_contract,
            &refund_contract,
        );

        // Freshly initialized: nothing paused, every contract at schema version 1.
        let status = client.get_system_status();
        assert!(!status.payment_paused);
        assert_eq!(status.payment_schema_version, 1);
        assert!(!status.escrow_paused);
        assert_eq!(status.escrow_schema_version, 1);
        assert!(!status.refund_paused);
        assert_eq!(status.refund_schema_version, 1);

        // Pause the payment contract and advance the refund schema version.
        let reason = String::from_str(&env, "status check");
        let payment = PaymentContractClient::new(&env, &payment_contract);
        payment.pause_contract(&pauser, &reason);

        let refund = RefundContractClient::new(&env, &refund_contract);
        refund.migrate_schema(&pauser, &2);

        // The unified report reflects both changes in a single query.
        let status = client.get_system_status();
        assert!(status.payment_paused);
        assert_eq!(status.payment_schema_version, 1);
        assert!(!status.escrow_paused);
        assert_eq!(status.escrow_schema_version, 1);
        assert!(!status.refund_paused);
        assert_eq!(status.refund_schema_version, 2);

        // Unpausing clears the flag in the report.
        payment.unpause_contract(&pauser);
        let status = client.get_system_status();
        assert!(!status.payment_paused);
    }

    #[test]
    fn test_initialize_and_pause_all() {
        let env = Env::default();
        env.mock_all_auths();

        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(&env, &admin_contract_id);

        let admin = Address::generate(&env);
        let pauser = Address::generate(&env);
        let payment_contract = setup_payment(&env, &pauser);
        let escrow_contract = setup_escrow(&env, &pauser);
        let refund_contract = setup_refund(&env, &pauser);

        client.initialize(
            &admin,
            &pauser,
            &payment_contract,
            &escrow_contract,
            &refund_contract,
        );

        let reason = String::from_str(&env, "security incident");
        client.emergency_pause_all(&pauser, &reason);
        client.emergency_unpause_all(&pauser);
    }

    #[test]
    fn test_contract_address_rotation_timelock() {
        let env = Env::default();
        env.mock_all_auths();

        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(&env, &admin_contract_id);

        let admin = Address::generate(&env);
        let pauser = Address::generate(&env);
        let payment_contract = setup_payment(&env, &pauser);
        let escrow_contract = setup_escrow(&env, &pauser);
        let refund_contract = setup_refund(&env, &pauser);

        client.initialize(
            &admin,
            &pauser,
            &payment_contract,
            &escrow_contract,
            &refund_contract,
        );

        // Propose a rotation of the payment contract.
        env.ledger().set_timestamp(1_000);
        let new_payment_contract = setup_payment(&env, &pauser);
        client.set_payment_contract(&admin, &new_payment_contract);

        // The pending rotation is recorded, but the stored address is unchanged.
        let pending = client
            .get_pending_rotation(&ContractTarget::Payment)
            .unwrap();
        assert_eq!(pending.new_address, new_payment_contract);
        assert_eq!(pending.proposed_by, admin);
        assert_eq!(pending.proposed_at, 1_000);

        // Executing before the 48-hour timelock elapses must fail.
        let result = client.try_execute_payment_rotation(&admin);
        assert_eq!(result, Err(Ok(Error::RotationTimelockActive)));

        // The stored address must still be the original one: pausing through
        // the admin contract affects the original contract, not the proposed
        // one.
        let reason = String::from_str(&env, "timelock check");
        client.emergency_pause_all(&pauser, &reason);
        let original = PaymentContractClient::new(&env, &payment_contract);
        assert!(original.get_pause_state().globally_paused);
        let proposed = PaymentContractClient::new(&env, &new_payment_contract);
        assert!(!proposed.get_pause_state().globally_paused);
        client.emergency_unpause_all(&pauser);

        // One second before the timelock elapses, execution still fails.
        env.ledger()
            .set_timestamp(1_000 + CONTRACT_ROTATION_TIMELOCK - 1);
        let result = client.try_execute_payment_rotation(&admin);
        assert_eq!(result, Err(Ok(Error::RotationTimelockActive)));

        // Once the timelock has elapsed, execution succeeds.
        env.ledger()
            .set_timestamp(1_000 + CONTRACT_ROTATION_TIMELOCK);
        client.execute_payment_rotation(&admin);

        // The ContractTargetUpdated event must have been emitted. This is
        // checked immediately after execution because Soroban test
        // environments reset the observable event buffer when further contract
        // invocations (such as the storage queries below) occur.
        assert!(env.events().all().iter().any(|e| {
            Symbol::try_from_val(&env, &e.1.get(0).unwrap_or_default()).ok()
                == Some(Symbol::new(&env, "contract_target_updated"))
        }));

        // The pending rotation is cleared.
        assert!(client
            .get_pending_rotation(&ContractTarget::Payment)
            .is_none());

        // The rotation took effect: pausing now hits the new contract.
        client.emergency_pause_all(&pauser, &reason);
        let proposed = PaymentContractClient::new(&env, &new_payment_contract);
        assert!(proposed.get_pause_state().globally_paused);
        client.emergency_unpause_all(&pauser);
    }

    #[test]
    fn test_cancel_proposed_rotation() {
        let env = Env::default();
        env.mock_all_auths();

        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(&env, &admin_contract_id);

        let admin = Address::generate(&env);
        let pauser = Address::generate(&env);
        let payment_contract = setup_payment(&env, &pauser);
        let escrow_contract = setup_escrow(&env, &pauser);
        let refund_contract = setup_refund(&env, &pauser);

        client.initialize(
            &admin,
            &pauser,
            &payment_contract,
            &escrow_contract,
            &refund_contract,
        );

        // Canceling a rotation that was never proposed fails.
        let result = client.try_cancel_proposed_rotation(&admin, &ContractTarget::Escrow);
        assert_eq!(result, Err(Ok(Error::RotationNotPending)));

        // Propose and then cancel a rotation.
        let new_escrow_contract = setup_escrow(&env, &pauser);
        client.set_escrow_contract(&admin, &new_escrow_contract);
        assert!(client
            .get_pending_rotation(&ContractTarget::Escrow)
            .is_some());

        client.cancel_proposed_rotation(&admin, &ContractTarget::Escrow);
        assert!(client
            .get_pending_rotation(&ContractTarget::Escrow)
            .is_none());

        // After cancellation the stored address is unchanged and a new
        // rotation can be proposed.
        let reason = String::from_str(&env, "post-cancel check");
        client.emergency_pause_all(&pauser, &reason);
        let escrow = EscrowContractClient::new(&env, &escrow_contract);
        assert!(escrow.get_pause_state().globally_paused);
        client.emergency_unpause_all(&pauser);

        let newer_escrow_contract = setup_escrow(&env, &pauser);
        client.set_escrow_contract(&admin, &newer_escrow_contract);
        let pending = client
            .get_pending_rotation(&ContractTarget::Escrow)
            .unwrap();
        assert_eq!(pending.new_address, newer_escrow_contract);
    }

    #[test]
    fn test_rotation_rejects_duplicate_proposal() {
        let env = Env::default();
        env.mock_all_auths();

        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(&env, &admin_contract_id);

        let admin = Address::generate(&env);
        let pauser = Address::generate(&env);
        let payment_contract = setup_payment(&env, &pauser);
        let escrow_contract = setup_escrow(&env, &pauser);
        let refund_contract = setup_refund(&env, &pauser);

        client.initialize(
            &admin,
            &pauser,
            &payment_contract,
            &escrow_contract,
            &refund_contract,
        );

        let new_refund_contract = setup_refund(&env, &pauser);
        client.set_refund_contract(&admin, &new_refund_contract);

        // A second proposal for the same target while one is pending fails.
        let another_refund_contract = setup_refund(&env, &pauser);
        let result = client.try_set_refund_contract(&admin, &another_refund_contract);
        assert_eq!(result, Err(Ok(Error::RotationAlreadyPending)));

        // Proposing a rotation for a different target still works.
        let new_payment_contract = setup_payment(&env, &pauser);
        client.set_payment_contract(&admin, &new_payment_contract);
        assert!(client
            .get_pending_rotation(&ContractTarget::Payment)
            .is_some());
    }
}
