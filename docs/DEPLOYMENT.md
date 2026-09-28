# Deploying Cypher GridPay Smart Contracts

This guide walks you through building, setting up identities, funding accounts, and deploying all four Cypher GridPay smart contracts across the `core` and `orchestrator` workspaces to Stellar Testnet (or local quickstart) using the **Stellar CLI**.

---

## Architecture & Workspaces

Cypher GridPay contracts are divided into two distinct Cargo workspaces:

1. **`core` workspace** (`core/`):
   - **Payment Contract** (`core/contracts/payment`): Handles invoices, payment processing, fee accounting, and settlement delays.
   - **Escrow Contract** (`core/contracts/escrow`): Manages conditional escrow locks, multi-sig approvals, dispute holds, and clawbacks.
   - **Refund Contract** (`core/contracts/refund`): Implements customer dispute resolution, appeals, arbitrator registries, and auto-refund triggers.

2. **`orchestrator` workspace** (`orchestrator/`):
   - **Admin Orchestrator Contract** (`orchestrator/contracts/admin`): Cross-contract coordinator providing unified administrative controls, emergency pausing, and component contract address registry.

---

## Deployment Order

Because the Admin Orchestrator references the deployed addresses of the core contracts, deployment **must** proceed in this strict sequential order:

```
Step 1: Payment Contract   (core)
Step 2: Escrow Contract    (core)
Step 3: Refund Contract    (core)
Step 4: Admin Orchestrator (orchestrator)
Step 5: Initialize Admin Orchestrator with Step 1-3 addresses
```

---

## Prerequisites

1. **Rust & `wasm32` Target**:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

2. **Stellar CLI**:
   ```bash
   cargo install --locked stellar-cli --features opt
   ```

3. **Stellar Network Configuration**:
   ```bash
   # Add Testnet
   stellar network add --global testnet \
     --rpc-url "https://soroban-testnet.stellar.org:443" \
     --network-passphrase "Test SDF Network ; September 2015"
   ```

---

## Step 1: Identity & Key Management

Generate and fund the deployment admin identity:

```bash
# Generate deployer keypair
stellar keys generate --network testnet deployer

# Fund deployer account via Testnet Friendbot
stellar keys fund deployer --network testnet

# Verify balance
stellar keys balance deployer --network testnet
```

---

## Step 2: Build WASM Artifacts Across Workspaces

Compile optimized WASM binaries for all contracts:

```bash
# Build core contracts (Payment, Escrow, Refund)
cd core
stellar contract build
cd ..

# Build orchestrator contract (Admin)
cd orchestrator
stellar contract build
cd ..

# Alternatively, build both via root Makefile:
make build
```

Compiled WASMs will be located at:
- `target/wasm32-unknown-unknown/release/payments.wasm`
- `target/wasm32-unknown-unknown/release/escrow.wasm`
- `target/wasm32-unknown-unknown/release/refund.wasm`
- `target/wasm32-unknown-unknown/release/admin.wasm`

Verify each binary does not exceed the 256 KB (262,144 bytes) limit via:
```bash
make check-size
```

---

## Step 3: Sequential Contract Deployment

### 1. Deploy Payment Contract
```bash
PAYMENT_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/payments.wasm \
  --source deployer \
  --network testnet)

echo "Payment Contract Address: $PAYMENT_ID"
```

### 2. Deploy Escrow Contract
```bash
ESCROW_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/escrow.wasm \
  --source deployer \
  --network testnet)

echo "Escrow Contract Address: $ESCROW_ID"
```

### 3. Deploy Refund Contract
```bash
REFUND_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/refund.wasm \
  --source deployer \
  --network testnet)

echo "Refund Contract Address: $REFUND_ID"
```

### 4. Deploy Admin Orchestrator Contract
```bash
ADMIN_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/admin.wasm \
  --source deployer \
  --network testnet)

echo "Admin Orchestrator Address: $ADMIN_ID"
```

---

## Step 4: Admin Orchestrator Initialization

Link the deployed contracts by initializing the Admin Orchestrator:

```bash
ADMIN_ADDRESS=$(stellar keys address deployer)
PAUSER_ADDRESS=$(stellar keys address deployer)

stellar contract invoke \
  --id "$ADMIN_ID" \
  --source deployer \
  --network testnet \
  -- \
  initialize \
  --admin "$ADMIN_ADDRESS" \
  --pauser "$PAUSER_ADDRESS" \
  --payment_contract "$PAYMENT_ID" \
  --escrow_contract "$ESCROW_ID" \
  --refund_contract "$REFUND_ID"
```

---

## Step 5: Verification & Health Checks

Verify that the orchestrator correctly reports contract addresses and status:

```bash
# Query Payment Contract registered address
stellar contract invoke \
  --id "$ADMIN_ID" \
  --source deployer \
  --network testnet \
  -- \
  get_payment_contract

# Query Escrow Contract registered address
stellar contract invoke \
  --id "$ADMIN_ID" \
  --source deployer \
  --network testnet \
  -- \
  get_escrow_contract

# Query Refund Contract registered address
stellar contract invoke \
  --id "$ADMIN_ID" \
  --source deployer \
  --network testnet \
  -- \
  get_refund_contract
```

---

## Deployment Manifest Record

Save the resulting contract addresses to an environment file or deployment manifest for client integrations:

```json
{
  "network": "testnet",
  "deployed_at": "2026-09-28T00:00:00Z",
  "contracts": {
    "payment": "C...",
    "escrow": "C...",
    "refund": "C...",
    "admin": "C..."
  }
}
```
