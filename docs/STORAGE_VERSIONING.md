# Storage Schema Versioning

Cypher GridPay Soroban smart contracts (`core/contracts/payment`, `core/contracts/refund`, `core/contracts/escrow`) and the orchestrator contracts (`orchestrator/contracts/admin`) implement an explicit storage schema versioning convention. This allows deployed contracts to track their data storage layout version on-chain and perform state migrations as stored data structures evolve over time.

---

## 📐 How Storage Schema Versions Are Tracked

Every contract tracks its schema version in instance storage under a dedicated storage key (`ConfigKey::SchemaVersion`, `SystemKey::SchemaVersion` or `DataKey::SchemaVersion`).

### Key Functions

1. **`get_schema_version(env: Env) -> u32`**
   - Returns the current schema version number stored in contract instance storage.
   - Defaults to `1` (`INITIAL_SCHEMA_VERSION`) if no custom version has been written yet.

2. **`migrate_schema(env: Env, admin: Address, target_version: u32) -> Result<(), Error>`**
   - Authorized admin-only function that updates the contract schema version to `target_version`.
   - Returns an error (`SchemaAlreadyAtTarget`) if the current stored version is already greater than or equal to `target_version`.
   - **Validates storage invariants before persisting the version (Issue #88).** Every data transformation registered for the version steps in `(current_version, target_version]` is executed *first*. `target_version` is written only after all transformations succeed, so a version can never be bumped on top of partially migrated state.

---

## 🛡️ Migration Invariant Checks (Issue #88)

`migrate_schema` never trusts the caller blindly. Before writing `target_version`, each contract runs the data migrations registered for the intermediate version steps:

| Contract | Version step | Transformation |
|---|---|---|
| `core/contracts/payment` | v1 → v2 | Re-indexes every payment in `1..=PaymentKey::Counter` into the customer index, the merchant index and the paged merchant index. |
| `core/contracts/refund` | v1 → v2 | Re-indexes every refund in `1..=RefundCounter` into the per-status index and the customer history index, and backfills `SystemKey::RefundRejectedAt` for rejected refunds. |
| `orchestrator/contracts/admin` | v1 → v2 | Re-validates the stored orchestrator configuration: the pauser and every child contract address must be present and the three child contracts must be distinct, otherwise an emergency pause could leave a role unmanaged. |

If a single entry cannot be read, or its stored id disagrees with the key it is stored under, the migration aborts with `SchemaMigrationFailed` and **the whole transaction is reverted**:

- the stored schema version stays at its previous value, so the migration can be retried after the corrupted entry is fixed;
- every write performed for the entries that *did* succeed is rolled back, so no half-migrated state survives.

```rust
// Pseudocode of the contract-side flow
if current >= target_version { return Err(SchemaAlreadyAtTarget); }
run_data_migrations(&env, current, target_version)?; // reverts on any failure
env.storage().instance().set(&schema_version_key, &target_version);
```

Reference tests: [`core/contracts/payment/src/test_schema_migration.rs`](../core/contracts/payment/src/test_schema_migration.rs) and [`core/contracts/refund/src/test_schema_migration.rs`](../core/contracts/refund/src/test_schema_migration.rs).

---

## 🛠️ Contributor Workflow: Changing Stored Data Shapes

When modifying an existing stored data structure (such as adding fields to a struct, modifying enum variants, or restructuring storage keys), contributors must adhere to the following workflow:

1. **Assess Breaking Changes**:
   - Determine if the change breaks backwards compatibility with existing on-chain data.
   - Adding non-optional fields or re-interpreting existing byte encodings requires a schema migration.

2. **Define Migration Logic**:
   - Update `migrate_schema()` in the relevant contract (e.g., [`core/contracts/payment/src/lib.rs`](../core/contracts/payment/src/lib.rs) or [`core/contracts/refund/src/lib.rs`](../core/contracts/refund/src/lib.rs)) to handle reading historical data shapes and writing upgraded data structures.
   - Register the per-version transformation in `run_data_migrations()` and return `SchemaMigrationFailed` when an entry cannot be transformed, so the failure reverts the transaction instead of silently bumping the version.

3. **Increment Target Schema Version**:
   - Ensure contract calls specify the new target version integer (`target_version > current_version`).

4. **Add & Update Unit Tests**:
   - Create or update contract tests to verify that:
     - `get_schema_version()` starts at `1` after contract `initialize()`.
     - `migrate_schema()` successfully increments the version when called by an authorized admin.
     - Calling `migrate_schema()` with a target version `<= current_version` fails with `SchemaAlreadyAtTarget`.
     - A failing entry migration leaves the stored version untouched and rolls back every partial write (Issue #88).

---

## 🧪 Reference Examples

The repository includes explicit tests demonstrating schema version initialization and migration enforcement:

- **Payment Contract**: [`core/contracts/payment/src/schema_version_test.rs`](../core/contracts/payment/src/schema_version_test.rs)
- **Refund Contract**: [`core/contracts/refund/src/schema_version_test.rs`](../core/contracts/refund/src/schema_version_test.rs)
- **Orchestrator Admin Contract**: [`orchestrator/contracts/admin/src/schema_version_test.rs`](../orchestrator/contracts/admin/src/schema_version_test.rs)
- **Migration invariant / rollback suites**: [`core/contracts/payment/src/test_schema_migration.rs`](../core/contracts/payment/src/test_schema_migration.rs) and [`core/contracts/refund/src/test_schema_migration.rs`](../core/contracts/refund/src/test_schema_migration.rs)

### Example Test Pattern

```rust
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
```
