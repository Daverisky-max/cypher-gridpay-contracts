#![no_std]
use escrow::EscrowContractClient;
use payments::PaymentContractClient;
use refund::RefundContractClient;
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};

#[contracterror]
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    // The stored schema version is already at or past the requested target.
    SchemaAlreadyAtTarget = 4,
    // A data migration step failed, so the schema version must not be bumped
    // (the whole transaction is reverted).
    SchemaMigrationFailed = 5,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Pauser,
    PaymentContract,
    EscrowContract,
    RefundContract,
    SchemaVersion,
}

/// Schema version written by `initialize`.
const INITIAL_SCHEMA_VERSION: u32 = 1;

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
        env.storage()
            .instance()
            .set(&DataKey::SchemaVersion, &INITIAL_SCHEMA_VERSION);

        Ok(())
    }

    /// Returns the current schema version of the contract storage.
    ///
    /// # Returns
    /// The stored schema version, or `1` (`INITIAL_SCHEMA_VERSION`) when the
    /// contract was initialized before schema versioning was introduced.
    pub fn get_schema_version(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::SchemaVersion)
            .unwrap_or(INITIAL_SCHEMA_VERSION)
    }

    /// Migrates the contract storage schema to a target version.
    ///
    /// Every data transformation registered for the versions between the
    /// current schema version and `target_version` is executed *before*
    /// `target_version` is written to storage, so the version can never be
    /// bumped on top of partially migrated state. If a single entry cannot be
    /// migrated the call returns `Error::SchemaMigrationFailed` and the whole
    /// transaction is reverted.
    ///
    /// # Parameters
    /// - `admin`: the admin authorizing the migration (must be the stored admin).
    /// - `target_version`: the schema version to migrate to.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` before `initialize`, `Error::Unauthorized`
    /// if the caller is not the stored admin, `Error::SchemaAlreadyAtTarget` when
    /// the stored version is already at or past the target, and
    /// `Error::SchemaMigrationFailed` if a data migration step failed.
    pub fn migrate_schema(env: Env, admin: Address, target_version: u32) -> Result<(), Error> {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        let current = Self::get_schema_version(env.clone());
        if current >= target_version {
            return Err(Error::SchemaAlreadyAtTarget);
        }

        // Run every data migration first; `target_version` is only persisted
        // once all transformations have completed successfully.
        Self::run_data_migrations(&env, current, target_version)?;

        env.storage()
            .instance()
            .set(&DataKey::SchemaVersion, &target_version);
        Ok(())
    }

    /// Runs the data transformations registered for every schema version step
    /// between `from_version` (exclusive) and `to_version` (inclusive).
    fn run_data_migrations(env: &Env, from_version: u32, to_version: u32) -> Result<(), Error> {
        let mut version = from_version;
        while version < to_version {
            let next_version = version + 1;
            // v2: the orchestrator configuration must stay complete and
            // resolvable, because every privileged entry point reads it.
            if next_version == 2 {
                Self::migrate_v1_to_v2(env)?;
            }
            version = next_version;
        }
        Ok(())
    }

    /// v1 -> v2 data migration: re-validates the stored orchestrator
    /// configuration. Every privileged function resolves the admin, pauser and
    /// child contract addresses from instance storage, so a missing entry or a
    /// duplicated child contract must abort the migration instead of silently
    /// bumping the version.
    fn migrate_v1_to_v2(env: &Env) -> Result<(), Error> {
        env.storage()
            .instance()
            .get::<_, Address>(&DataKey::Pauser)
            .ok_or(Error::SchemaMigrationFailed)?;
        let payment: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentContract)
            .ok_or(Error::SchemaMigrationFailed)?;
        let escrow: Address = env
            .storage()
            .instance()
            .get(&DataKey::EscrowContract)
            .ok_or(Error::SchemaMigrationFailed)?;
        let refund: Address = env
            .storage()
            .instance()
            .get(&DataKey::RefundContract)
            .ok_or(Error::SchemaMigrationFailed)?;

        // The three child contracts must be distinct, otherwise an emergency
        // pause would hit the same contract twice and leave a role unmanaged.
        if payment == escrow || payment == refund || escrow == refund {
            return Err(Error::SchemaMigrationFailed);
        }

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
        client.emergency_unpause_all(&pauser);
    }
}

#[cfg(test)]
mod schema_version_test;
