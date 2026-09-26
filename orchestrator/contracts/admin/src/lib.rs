#![no_std]
use escrow::EscrowContractClient;
use payments::PaymentContractClient;
use refund::RefundContractClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
};

#[contracterror]
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    UpgradeProposalNotFound = 4,
    UpgradeTimelockNotExpired = 5,
    InsufficientApprovals = 6,
    UpgradeAlreadyExecuted = 7,
    InvalidWasmHash = 8,
    ReentrancyDetected = 9,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Pauser,
    PaymentContract,
    EscrowContract,
    RefundContract,
    UpgradeProposal(u64),
    UpgradeProposalCounter,
    UpgradeTimelock,
    UpgradeRequiredApprovals,
    ReentrancyLock,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct UpgradeProposal {
    pub contract_address: Address,
    pub new_wasm_hash: BytesN<32>,
    pub proposed_at: u64,
    pub timelock_period: u64,
    pub approvals: Vec<Address>,
    pub executed: bool,
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

        // Reentrancy protection
        let lock_key = DataKey::ReentrancyLock;
        let locked: bool = env.storage().instance().get(&lock_key).unwrap_or(false);
        if locked {
            return Err(Error::ReentrancyDetected);
        }
        env.storage().instance().set(&lock_key, &true);

        let result = Self::do_pause_all(&env, &pauser, &reason);

        // Release lock
        env.storage().instance().set(&lock_key, &false);

        result
    }

    fn do_pause_all(env: &Env, pauser: &Address, reason: &String) -> Result<(), Error> {
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

        PaymentContractClient::new(env, &payment_contract).pause_contract(pauser, reason);
        EscrowContractClient::new(env, &escrow_contract).pause_contract(pauser, reason);
        RefundContractClient::new(env, &refund_contract).pause_contract(pauser, reason);

        // Emit audit event
        env.events().publish(
            (Symbol::new(env, "emergency_pause_all"), pauser.clone()),
            reason.clone(),
        );

        Ok(())
    }

    /// Unpauses the payment, escrow, and refund contracts in one Soroban call.
    /// Requires super-admin authorization (not just pauser).
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `justification`: a human-readable explanation for the emergency unpause.
    ///
    /// # Returns
    /// Returns `Ok(())` when all child contracts are unpaused successfully.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// and `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin.
    pub fn emergency_unpause_all(env: Env, admin: Address, justification: String) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        // Reentrancy protection
        let lock_key = DataKey::ReentrancyLock;
        let locked: bool = env.storage().instance().get(&lock_key).unwrap_or(false);
        if locked {
            return Err(Error::ReentrancyDetected);
        }
        env.storage().instance().set(&lock_key, &true);

        let result = Self::do_unpause_all(&env, &admin, &justification);

        // Release lock
        env.storage().instance().set(&lock_key, &false);

        result
    }

    fn do_unpause_all(env: &Env, admin: &Address, justification: &String) -> Result<(), Error> {
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

        PaymentContractClient::new(env, &payment_contract).unpause_contract(admin);
        EscrowContractClient::new(env, &escrow_contract).unpause_contract(admin);
        RefundContractClient::new(env, &refund_contract).unpause_contract(admin);

        // Emit audit event
        env.events().publish(
            (Symbol::new(env, "emergency_unpause_all"), admin.clone()),
            justification.clone(),
        );

        Ok(())
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

    /// Sets or updates the emergency pauser address.
    /// Only the admin can change the pauser role.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `pauser`: the new emergency pauser address.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// and `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin.
    pub fn set_emergency_pauser(env: Env, admin: Address, pauser: Address) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        env.storage().instance().set(&DataKey::Pauser, &pauser);

        // Emit audit event
        env.events().publish(
            (Symbol::new(&env, "set_emergency_pauser"), admin),
            pauser,
        );

        Ok(())
    }

    /// Configures the upgrade timelock period and required number of approvals.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `timelock_period`: the time in seconds that must pass before an upgrade can be executed.
    /// - `required_approvals`: the number of admin approvals required to execute an upgrade.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// and `Error::Unauthorized` if the provided admin address does not match the
    /// stored admin.
    pub fn configure_upgrade(
        env: Env,
        admin: Address,
        timelock_period: u64,
        required_approvals: u32,
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
            .set(&DataKey::UpgradeTimelock, &timelock_period);
        env.storage()
            .instance()
            .set(&DataKey::UpgradeRequiredApprovals, &required_approvals);

        Ok(())
    }

    /// Proposes a WASM bytecode upgrade for a target contract.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `contract_address`: the address of the contract to upgrade.
    /// - `new_wasm_hash`: the hash of the new WASM bytecode.
    ///
    /// # Returns
    /// Returns the proposal ID on success.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the stored admin,
    /// or `Error::InvalidWasmHash` if the wasm hash is zero.
    pub fn propose_upgrade(
        env: Env,
        admin: Address,
        contract_address: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        // Validate wasm hash is not zero
        let zero_hash = BytesN::<32>::from_array(&env, &[0u8; 32]);
        if new_wasm_hash == zero_hash {
            return Err(Error::InvalidWasmHash);
        }

        let timelock_period: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeTimelock)
            .unwrap_or(86400); // Default 24 hours

        let counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeProposalCounter)
            .unwrap_or(0);

        let proposal = UpgradeProposal {
            contract_address: contract_address.clone(),
            new_wasm_hash: new_wasm_hash.clone(),
            proposed_at: env.ledger().timestamp(),
            timelock_period,
            approvals: Vec::new(&env),
            executed: false,
        };

        env.storage()
            .instance()
            .set(&DataKey::UpgradeProposal(counter), &proposal);
        env.storage()
            .instance()
            .set(&DataKey::UpgradeProposalCounter, &(counter + 1));

        Ok(counter)
    }

    /// Approves a pending upgrade proposal.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `proposal_id`: the ID of the upgrade proposal to approve.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the stored admin,
    /// `Error::UpgradeProposalNotFound` if the proposal does not exist,
    /// or `Error::UpgradeAlreadyExecuted` if the proposal has already been executed.
    pub fn approve_upgrade(env: Env, admin: Address, proposal_id: u64) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        let mut proposal: UpgradeProposal = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeProposal(proposal_id))
            .ok_or(Error::UpgradeProposalNotFound)?;

        if proposal.executed {
            return Err(Error::UpgradeAlreadyExecuted);
        }

        // Check if admin already approved
        for i in 0..proposal.approvals.len() {
            if proposal.approvals.get(i).unwrap() == admin {
                return Ok(()); // Already approved, idempotent
            }
        }

        proposal.approvals.push_back(admin);
        env.storage()
            .instance()
            .set(&DataKey::UpgradeProposal(proposal_id), &proposal);

        Ok(())
    }

    /// Executes an approved upgrade proposal after the timelock has expired.
    ///
    /// # Parameters
    /// - `admin`: the admin address that must be authorized.
    /// - `proposal_id`: the ID of the upgrade proposal to execute.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the admin contract has not been initialized,
    /// `Error::Unauthorized` if the provided admin address does not match the stored admin,
    /// `Error::UpgradeProposalNotFound` if the proposal does not exist,
    /// `Error::UpgradeTimelockNotExpired` if the timelock period has not passed,
    /// `Error::InsufficientApprovals` if the required number of approvals has not been met,
    /// or `Error::UpgradeAlreadyExecuted` if the proposal has already been executed.
    pub fn execute_upgrade(env: Env, admin: Address, proposal_id: u64) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        let mut proposal: UpgradeProposal = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeProposal(proposal_id))
            .ok_or(Error::UpgradeProposalNotFound)?;

        if proposal.executed {
            return Err(Error::UpgradeAlreadyExecuted);
        }

        // Check timelock
        let current_time = env.ledger().timestamp();
        let unlock_time = proposal.proposed_at + proposal.timelock_period;
        if current_time < unlock_time {
            return Err(Error::UpgradeTimelockNotExpired);
        }

        // Check approvals
        let required_approvals: u32 = env
            .storage()
            .instance()
            .get(&DataKey::UpgradeRequiredApprovals)
            .unwrap_or(1);
        if proposal.approvals.len() < required_approvals as u64 {
            return Err(Error::InsufficientApprovals);
        }

        // Mark as executed
        proposal.executed = true;
        env.storage()
            .instance()
            .set(&DataKey::UpgradeProposal(proposal_id), &proposal);

        // Execute the upgrade
        env.deployer()
            .update_current_contract_wasm(&proposal.contract_address, &proposal.new_wasm_hash);

        Ok(())
    }

    /// Returns the details of an upgrade proposal.
    ///
    /// # Parameters
    /// - `proposal_id`: the ID of the upgrade proposal to query.
    ///
    /// # Returns
    /// Returns the `UpgradeProposal` struct.
    ///
    /// # Errors
    /// Returns `Error::UpgradeProposalNotFound` if the proposal does not exist.
    pub fn get_upgrade_proposal(env: Env, proposal_id: u64) -> Result<UpgradeProposal, Error> {
        env.storage()
            .instance()
            .get(&DataKey::UpgradeProposal(proposal_id))
            .ok_or(Error::UpgradeProposalNotFound)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

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
        let justification = String::from_str(&env, "issue resolved");
        client.emergency_unpause_all(&admin, &justification);
    }

    #[test]
    fn test_pauser_cannot_unpause() {
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

        // Pause first
        let reason = String::from_str(&env, "security incident");
        client.emergency_pause_all(&pauser, &reason);

        // Pauser tries to unpause - should fail
        let justification = String::from_str(&env, "trying to unpause");
        let result = client.try_emergency_unpause_all(&pauser, &justification);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_emergency_pauser() {
        let env = Env::default();
        env.mock_all_auths();

        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(&env, &admin_contract_id);

        let admin = Address::generate(&env);
        let pauser = Address::generate(&env);
        let new_pauser = Address::generate(&env);
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

        // Admin sets new pauser
        client.set_emergency_pauser(&admin, &new_pauser);

        // New pauser can pause
        let reason = String::from_str(&env, "test pause");
        client.emergency_pause_all(&new_pauser, &reason);

        // Old pauser can no longer pause
        let result = client.try_emergency_pause_all(&pauser, &reason);
        assert!(result.is_err());
    }

    #[test]
    fn test_reentrancy_protection() {
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

        // Manually set the reentrancy lock
        env.as_contract(&admin_contract_id, || {
            env.storage()
                .instance()
                .set(&DataKey::ReentrancyLock, &true);
        });

        // Try to pause - should fail due to reentrancy lock
        let reason = String::from_str(&env, "test reentrancy");
        let result = client.try_emergency_pause_all(&pauser, &reason);
        assert!(result.is_err());

        // Try to unpause - should also fail
        let justification = String::from_str(&env, "test reentrancy");
        let result = client.try_emergency_unpause_all(&admin, &justification);
        assert!(result.is_err());
    }

    #[test]
    fn test_upgrade_proposal_lifecycle() {
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

        // Configure upgrade with 0 timelock and 1 approval for testing
        client.configure_upgrade(&admin, &0, &1);

        // Create a dummy wasm hash
        let wasm_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);

        // Propose upgrade
        let proposal_id = client.propose_upgrade(&admin, &payment_contract, &wasm_hash);
        assert_eq!(proposal_id, 0);

        // Verify proposal exists
        let proposal = client.get_upgrade_proposal(&proposal_id);
        assert_eq!(proposal.contract_address, payment_contract);
        assert_eq!(proposal.new_wasm_hash, wasm_hash);
        assert!(!proposal.executed);

        // Approve the upgrade
        client.approve_upgrade(&admin, &proposal_id);

        // Execute the upgrade
        client.execute_upgrade(&admin, &proposal_id);

        // Verify proposal is marked as executed
        let proposal = client.get_upgrade_proposal(&proposal_id);
        assert!(proposal.executed);
    }

    #[test]
    fn test_upgrade_timelock_enforcement() {
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

        // Configure upgrade with 1 hour timelock
        client.configure_upgrade(&admin, &3600, &1);

        let wasm_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
        let proposal_id = client.propose_upgrade(&admin, &payment_contract, &wasm_hash);
        client.approve_upgrade(&admin, &proposal_id);

        // Try to execute before timelock expires - should fail
        let result = client.try_execute_upgrade(&admin, &proposal_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_upgrade_multisig_enforcement() {
        let env = Env::default();
        env.mock_all_auths();

        let admin_contract_id = env.register(AdminContract, ());
        let client = AdminContractClient::new(&env, &admin_contract_id);

        let admin = Address::generate(&env);
        let admin2 = Address::generate(&env);
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

        // Configure upgrade with 2 required approvals
        client.configure_upgrade(&admin, &0, &2);

        let wasm_hash = BytesN::<32>::from_array(&env, &[1u8; 32]);
        let proposal_id = client.propose_upgrade(&admin, &payment_contract, &wasm_hash);
        client.approve_upgrade(&admin, &proposal_id);

        // Try to execute with only 1 approval - should fail
        let result = client.try_execute_upgrade(&admin, &proposal_id);
        assert!(result.is_err());

        // Add second approval
        client.approve_upgrade(&admin2, &proposal_id);

        // Now execution should succeed
        client.execute_upgrade(&admin, &proposal_id);
    }
}
