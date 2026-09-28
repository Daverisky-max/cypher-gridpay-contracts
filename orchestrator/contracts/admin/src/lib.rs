#![no_std]
use escrow::EscrowContractClient;
use payments::PaymentContractClient;
use refund::RefundContractClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, IntoVal, String, Symbol, Vec,
};

#[contracterror]
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    /// Issue #73: a child contract's `pause_contract` / `unpause_contract` call
    /// returned an error. The whole transaction is reverted, so no child is left
    /// in a different state from its siblings.
    ChildContractCallFailed = 4,
    /// Issue #73: a pre-flight check found a child already in the target state.
    /// Reverting before the first mutation keeps the set of contracts consistent.
    ChildAlreadyInTargetState = 5,
    /// Issue #73: post-conditions were not met after every child reported success.
    ChildStateVerificationFailed = 6,
}

/// Which managed contract a coordination status refers to.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedContract {
    Payment,
    Escrow,
    Refund,
}

/// Issue #73: per-child pause state observed by the orchestrator, so operators
/// and monitoring can assert that the managed contracts are in lockstep instead
/// of guessing from three separate ledgers.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractPauseStatus {
    pub contract: ManagedContract,
    pub address: Address,
    pub paused: bool,
    /// `true` when `get_pause_state` could not be read back from this child.
    pub reachable: bool,
}

/// Issue #73: aggregate view of the managed contracts' pause state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoordinationStatus {
    pub payment_paused: bool,
    pub escrow_paused: bool,
    pub refund_paused: bool,
    /// `true` only when all three children report the same pause state and were
    /// all readable.
    pub consistent: bool,
    pub children: Vec<ContractPauseStatus>,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Pauser,
    PaymentContract,
    EscrowContract,
    RefundContract,
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
    /// Issue #73: the three contracts are paused as a single all-or-nothing unit.
    /// A failure in any child (`try_pause_contract` returning `Err`, or the
    /// post-condition read-back disagreeing) aborts the whole transaction, so the
    /// platform can never be left in a split state where, say, payment is paused
    /// while escrow is still accepting operations.
    ///
    /// The sequence is:
    /// 1. read all three addresses (bailing out before touching any child if the
    ///    orchestrator is not fully initialized),
    /// 2. pre-flight: no child may already be paused, otherwise the set would
    ///    already be inconsistent,
    /// 3. pause each child, converting any child error into a revert,
    /// 4. verify all three report `globally_paused`, otherwise revert.
    ///
    /// # Parameters
    /// - `pauser`: the pauser address that must be authorized.
    /// - `reason`: a human-readable explanation for the emergency pause.
    ///
    /// # Returns
    /// Returns `Ok(())` when all child contracts are paused successfully and the
    /// post-condition holds.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided pauser address does not match the
    /// stored pauser, `Error::ChildAlreadyInTargetState` if any child is already
    /// paused, `Error::ChildContractCallFailed` if a child refuses to pause, and
    /// `Error::ChildStateVerificationFailed` if the read-back does not show all
    /// three children paused.
    pub fn emergency_pause_all(env: Env, pauser: Address, reason: String) -> Result<(), Error> {
        pauser.require_auth();

        let (payment_contract, escrow_contract, refund_contract) = Self::managed_contracts(&env)?;
        Self::require_pauser(&env, &pauser)?;

        // Pre-flight (2): refuse to touch any child if the set is already
        // inconsistent. Reverting here - before the first mutation - guarantees we
        // never make a partially consistent set worse.
        let before =
            Self::read_pause_status(&env, &payment_contract, &escrow_contract, &refund_contract);
        if before.payment_paused || before.escrow_paused || before.refund_paused {
            return Err(Error::ChildAlreadyInTargetState);
        }

        // (3) Pause every child, reverting on the first error so the platform is
        // never left half-paused.
        PaymentContractClient::new(&env, &payment_contract)
            .try_pause_contract(&pauser, &reason)
            .map_err(|_| Error::ChildContractCallFailed)?;
        EscrowContractClient::new(&env, &escrow_contract)
            .try_pause_contract(&pauser, &reason)
            .map_err(|_| Error::ChildContractCallFailed)?;
        RefundContractClient::new(&env, &refund_contract)
            .try_pause_contract(&pauser, &reason)
            .map_err(|_| Error::ChildContractCallFailed)?;

        // (4) Post-conditions: all three must now agree.
        let after =
            Self::read_pause_status(&env, &payment_contract, &escrow_contract, &refund_contract);
        if !after.payment_paused || !after.escrow_paused || !after.refund_paused {
            return Err(Error::ChildStateVerificationFailed);
        }

        Ok(())
    }

    /// Unpauses the payment, escrow, and refund contracts in one Soroban call.
    ///
    /// Issue #73: mirrors [`emergency_pause_all`](Self::emergency_pause_all) so
    /// unpausing is atomic too - a child that refuses to unpause reverts the
    /// whole transaction instead of leaving the platform half-live.
    ///
    /// # Parameters
    /// - `pauser`: the pauser address that must be authorized.
    ///
    /// # Returns
    /// Returns `Ok(())` when all child contracts are unpaused successfully and the
    /// post-condition holds.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the pauser does not match the stored pauser,
    /// `Error::ChildAlreadyInTargetState` if any child is already unpaused,
    /// `Error::ChildContractCallFailed` if a child refuses to unpause, and
    /// `Error::ChildStateVerificationFailed` if the read-back does not show all
    /// three children unpaused.
    pub fn emergency_unpause_all(env: Env, pauser: Address) -> Result<(), Error> {
        pauser.require_auth();

        let (payment_contract, escrow_contract, refund_contract) = Self::managed_contracts(&env)?;
        Self::require_pauser(&env, &pauser)?;

        let before =
            Self::read_pause_status(&env, &payment_contract, &escrow_contract, &refund_contract);
        if !before.payment_paused || !before.escrow_paused || !before.refund_paused {
            return Err(Error::ChildAlreadyInTargetState);
        }

        PaymentContractClient::new(&env, &payment_contract)
            .try_unpause_contract(&pauser)
            .map_err(|_| Error::ChildContractCallFailed)?;
        EscrowContractClient::new(&env, &escrow_contract)
            .try_unpause_contract(&pauser)
            .map_err(|_| Error::ChildContractCallFailed)?;
        RefundContractClient::new(&env, &refund_contract)
            .try_unpause_contract(&pauser)
            .map_err(|_| Error::ChildContractCallFailed)?;

        let after =
            Self::read_pause_status(&env, &payment_contract, &escrow_contract, &refund_contract);
        if after.payment_paused || after.escrow_paused || after.refund_paused {
            return Err(Error::ChildStateVerificationFailed);
        }

        Ok(())
    }

    /// Issue #73: returns the pause state of every managed contract plus an
    /// aggregate `consistent` flag.
    ///
    /// Monitoring and incident response can call this to confirm the platform is
    /// in lockstep (all paused, or all live) instead of inferring it from three
    /// independent contract states.
    ///
    /// # Returns
    /// `CoordinationStatus` with one `ContractPauseStatus` per managed contract.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized.
    pub fn get_coordination_status(env: Env) -> Result<CoordinationStatus, Error> {
        let (payment_contract, escrow_contract, refund_contract) = Self::managed_contracts(&env)?;
        Ok(Self::read_pause_status(
            &env,
            &payment_contract,
            &escrow_contract,
            &refund_contract,
        ))
    }

    /// Loads the three managed contract addresses, failing before any child is
    /// touched if the orchestrator is not fully initialized.
    fn managed_contracts(env: &Env) -> Result<(Address, Address, Address), Error> {
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
        Ok((payment_contract, escrow_contract, refund_contract))
    }

    /// Verifies `pauser` matches the stored pauser.
    fn require_pauser(env: &Env, pauser: &Address) -> Result<(), Error> {
        let stored_pauser: Address = env
            .storage()
            .instance()
            .get(&DataKey::Pauser)
            .ok_or(Error::NotInitialized)?;
        if *pauser != stored_pauser {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    /// Reads `globally_paused` from each child. A child that cannot be read is
    /// reported as `reachable: false` and never counted as agreeing.
    fn read_pause_status(
        env: &Env,
        payment_contract: &Address,
        escrow_contract: &Address,
        refund_contract: &Address,
    ) -> CoordinationStatus {
        let payment_paused = Self::child_paused(env, payment_contract);
        let escrow_paused = Self::child_paused(env, escrow_contract);
        let refund_paused = Self::child_paused(env, refund_contract);

        let mut children: Vec<ContractPauseStatus> = Vec::new(env);
        children.push_back(ContractPauseStatus {
            contract: ManagedContract::Payment,
            address: payment_contract.clone(),
            paused: payment_paused.unwrap_or(false),
            reachable: payment_paused.is_some(),
        });
        children.push_back(ContractPauseStatus {
            contract: ManagedContract::Escrow,
            address: escrow_contract.clone(),
            paused: escrow_paused.unwrap_or(false),
            reachable: escrow_paused.is_some(),
        });
        children.push_back(ContractPauseStatus {
            contract: ManagedContract::Refund,
            address: refund_contract.clone(),
            paused: refund_paused.unwrap_or(false),
            reachable: refund_paused.is_some(),
        });

        let all_reachable =
            payment_paused.is_some() && escrow_paused.is_some() && refund_paused.is_some();
        let consistent =
            all_reachable && payment_paused == escrow_paused && escrow_paused == refund_paused;

        CoordinationStatus {
            payment_paused: payment_paused.unwrap_or(false),
            escrow_paused: escrow_paused.unwrap_or(false),
            refund_paused: refund_paused.unwrap_or(false),
            consistent,
            children,
        }
    }

    /// `get_pause_state` on a child, flattened to `globally_paused`.
    fn child_paused(env: &Env, contract: &Address) -> Option<bool> {
        let args = ().into_val(env);
        let state: payments::PauseState = env
            .try_invoke_contract::<payments::PauseState, soroban_sdk::InvokeError>(
                contract,
                &Symbol::new(env, "get_pause_state"),
                args,
            )
            .ok()?
            .ok()?;
        Some(state.globally_paused)
    }

    /// Updates the stored payment contract address.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `payment_contract`: the new payment contract address.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// and `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin.
    pub fn set_payment_contract(
        env: Env,
        admin: Address,
        payment_contract: Address,
    ) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        env.storage()
            .instance()
            .set(&DataKey::PaymentContract, &payment_contract);

        Ok(())
    }

    /// Updates the stored escrow contract address.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `escrow_contract`: the new escrow contract address.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// and `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin.
    pub fn set_escrow_contract(
        env: Env,
        admin: Address,
        escrow_contract: Address,
    ) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        env.storage()
            .instance()
            .set(&DataKey::EscrowContract, &escrow_contract);

        Ok(())
    }

    /// Updates the stored refund contract address.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `refund_contract`: the new refund contract address.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// and `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin.
    pub fn set_refund_contract(
        env: Env,
        admin: Address,
        refund_contract: Address,
    ) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        env.storage()
            .instance()
            .set(&DataKey::RefundContract, &refund_contract);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        contract, contracterror, contractimpl, contracttype, testutils::Address as _,
    };

    /// Issue #73: stand-in for a child contract that refuses to pause. It exposes
    /// the same `pause_contract` / `unpause_contract` / `get_pause_state` surface
    /// the orchestrator invokes, so a failure in the middle of the sequence can be
    /// simulated deterministically.
    #[contracterror]
    #[derive(Clone, Debug, PartialEq)]
    pub enum StubError {
        Rejected = 1,
    }

    #[contracttype]
    pub enum StubKey {
        RejectPause,
        Paused,
    }

    /// `set_reject_transitions(true)` makes `pause_contract` / `unpause_contract`
    /// fail. Because the
    /// orchestrator pauses payment first, wiring this into the escrow slot
    /// reproduces exactly the split-state window issue #73 describes.
    #[contract]
    pub struct FailingChildContract;

    #[contractimpl]
    impl FailingChildContract {
        pub fn set_reject_transitions(env: Env, reject: bool) {
            env.storage().instance().set(&StubKey::RejectPause, &reject);
        }

        pub fn pause_contract(env: Env, _admin: Address, _reason: String) -> Result<(), StubError> {
            if env
                .storage()
                .instance()
                .get::<StubKey, bool>(&StubKey::RejectPause)
                .unwrap_or(false)
            {
                return Err(StubError::Rejected);
            }
            env.storage().instance().set(&StubKey::Paused, &true);
            Ok(())
        }

        pub fn unpause_contract(env: Env, _admin: Address) -> Result<(), StubError> {
            if env
                .storage()
                .instance()
                .get::<StubKey, bool>(&StubKey::RejectPause)
                .unwrap_or(false)
            {
                return Err(StubError::Rejected);
            }
            env.storage().instance().set(&StubKey::Paused, &false);
            Ok(())
        }

        /// Returns the real `payments::PauseState` so the orchestrator's
        /// read-back is byte-compatible with a live child contract. Returning a
        /// different shape here would make the host abort on conversion rather
        /// than model a genuine "child refused" failure.
        pub fn get_pause_state(env: Env) -> payments::PauseState {
            let paused = env
                .storage()
                .instance()
                .get::<StubKey, bool>(&StubKey::Paused)
                .unwrap_or(false);
            payments::PauseState {
                globally_paused: paused,
                paused_functions: soroban_sdk::Vec::new(&env),
                paused_at: if paused { 1 } else { 0 },
                paused_by: env.current_contract_address(),
                pause_reason: String::from_str(&env, "stub"),
            }
        }
    }

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

    /// Registers a real child contract for every managed slot and returns the
    /// orchestrator client, the pauser, the admin, and the three children.
    fn setup(
        env: &Env,
    ) -> (
        AdminContractClient<'static>,
        Address,
        Address,
        Address,
        Address,
        Address,
    ) {
        env.mock_all_auths();
        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(env, &admin_contract_id);

        let admin = Address::generate(env);
        let pauser = Address::generate(env);
        let payment_contract = setup_payment(env, &pauser);
        let escrow_contract = setup_escrow(env, &pauser);
        let refund_contract = setup_refund(env, &pauser);

        client.initialize(
            &admin,
            &pauser,
            &payment_contract,
            &escrow_contract,
            &refund_contract,
        );
        (
            client,
            admin,
            pauser,
            payment_contract,
            escrow_contract,
            refund_contract,
        )
    }

    fn reason(env: &Env) -> String {
        String::from_str(env, "security incident")
    }

    fn is_globally_paused(env: &Env, child: &Address) -> bool {
        let args = ().into_val(env);
        let state: payments::PauseState =
            env.invoke_contract(child, &Symbol::new(env, "get_pause_state"), args);
        state.globally_paused
    }

    #[test]
    fn test_initialize_and_pause_all() {
        let env = Env::default();
        let (client, _admin, pauser, _, _, _) = setup(&env);

        client.emergency_pause_all(&pauser, &reason(&env));
        client.emergency_unpause_all(&pauser);
    }

    /// The happy path really does pause all three children, and the reported
    /// status agrees with what was read back from each child.
    #[test]
    fn test_pause_all_pauses_every_child() {
        let env = Env::default();
        let (client, _admin, pauser, payment, escrow, refund) = setup(&env);

        client.emergency_pause_all(&pauser, &reason(&env));

        assert!(is_globally_paused(&env, &payment));
        assert!(is_globally_paused(&env, &escrow));
        assert!(is_globally_paused(&env, &refund));

        let status = client.get_coordination_status();
        assert!(status.payment_paused);
        assert!(status.escrow_paused);
        assert!(status.refund_paused);
        assert!(status.consistent);
        assert_eq!(status.children.len(), 3);
    }

    /// Issue #73 regression: the escrow slot points at a contract that refuses to
    /// pause. The payment contract is paused *first*, so without an atomic revert
    /// the platform would be left split. The whole call must fail and the payment
    /// contract must be left unpaused.
    #[test]
    fn test_partial_pause_failure_reverts_every_child() {
        let env = Env::default();
        let (client, admin, pauser, payment, _, _) = setup(&env);

        let rejecting = env.register(FailingChildContract, ());
        FailingChildContractClient::new(&env, &rejecting).set_reject_transitions(&true);
        client.set_escrow_contract(&admin, &rejecting);

        let result = client.try_emergency_pause_all(&pauser, &reason(&env));
        assert_eq!(result, Err(Ok(Error::ChildContractCallFailed)));

        // Payment was paused before the failure; the revert must have rolled that
        // back too, so no contract is left half-paused.
        assert!(
            !is_globally_paused(&env, &payment),
            "payment must not stay paused after a failed pause_all"
        );
    }

    /// The mirror-image split-state window on the unpause path.
    #[test]
    fn test_partial_unpause_failure_reverts_every_child() {
        let env = Env::default();
        let (client, admin, pauser, payment, _, _) = setup(&env);

        client.emergency_pause_all(&pauser, &reason(&env));

        // Stand in a child that is already paused but refuses to unpause, so the
        // pre-flight check passes and the failure happens mid-sequence - the
        // exact moment payment has already been unpaused.
        let rejecting = env.register(FailingChildContract, ());
        let stub = FailingChildContractClient::new(&env, &rejecting);
        stub.pause_contract(&pauser, &reason(&env));
        stub.set_reject_transitions(&true);
        client.set_escrow_contract(&admin, &rejecting);

        let result = client.try_emergency_unpause_all(&pauser);
        assert_eq!(result, Err(Ok(Error::ChildContractCallFailed)));

        assert!(
            is_globally_paused(&env, &payment),
            "payment must stay paused after a failed unpause_all"
        );
    }

    /// Pre-flight guard: if one child is already paused the orchestrator refuses
    /// before mutating anything, so an already-inconsistent set is never made
    /// worse by a second partial attempt.
    #[test]
    fn test_pause_all_rejects_already_paused_child() {
        let env = Env::default();
        let (client, _admin, pauser, payment, escrow, _) = setup(&env);

        EscrowContractClient::new(&env, &escrow).pause_contract(&pauser, &reason(&env));

        let result = client.try_emergency_pause_all(&pauser, &reason(&env));
        assert_eq!(result, Err(Ok(Error::ChildAlreadyInTargetState)));

        // Payment was never touched.
        assert!(!is_globally_paused(&env, &payment));
        let status = client.get_coordination_status();
        assert!(status.escrow_paused);
        assert!(!status.consistent);
    }

    /// Unpausing an already-live platform is a no-op request, not a reason to run
    /// a half-unpause sequence.
    #[test]
    fn test_unpause_all_rejects_already_live_child() {
        let env = Env::default();
        let (client, _admin, pauser, _, _, _) = setup(&env);

        let result = client.try_emergency_unpause_all(&pauser);
        assert_eq!(result, Err(Ok(Error::ChildAlreadyInTargetState)));
    }

    /// An unreachable child is reported, never silently treated as agreement.
    #[test]
    fn test_coordination_status_flags_unreachable_child() {
        let env = Env::default();
        let (client, admin, _pauser, _, _, _) = setup(&env);

        client.set_refund_contract(&admin, &Address::generate(&env));

        let status = client.get_coordination_status();
        assert!(!status.consistent);
        let refund = status.children.get(2).unwrap();
        assert_eq!(refund.contract, ManagedContract::Refund);
        assert!(!refund.reachable);
    }

    /// Pre-existing behaviour that must not regress.
    #[test]
    fn test_unauthorized_pauser_is_rejected() {
        let env = Env::default();
        let (client, _admin, _pauser, _, _, _) = setup(&env);
        let stranger = Address::generate(&env);

        let result = client.try_emergency_pause_all(&stranger, &reason(&env));
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    }

    /// A partially configured orchestrator must fail before touching any child.
    #[test]
    fn test_pause_all_requires_full_initialization() {
        let env = Env::default();
        env.mock_all_auths();
        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(&env, &admin_contract_id);

        let admin = Address::generate(&env);
        let pauser = Address::generate(&env);
        let stranger = Address::generate(&env);

        let result = client.try_emergency_pause_all(&pauser, &reason(&env));
        assert_eq!(result, Err(Ok(Error::NotInitialized)));

        // Initialized with a bogus escrow slot: the status read reports the child
        // as unreachable rather than assuming it is fine.
        client.initialize(&admin, &pauser, &stranger, &stranger, &stranger);
        let status = client.get_coordination_status();
        assert!(!status.consistent);
    }
}
