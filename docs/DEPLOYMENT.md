# Deploying Smart Contracts to Stellar Testnet

This guide walks you through building, setting up identities, funding accounts, and deploying all four Cypher GridPay smart contracts (`admin`, `payment`, `escrow`, and `refund`) to the Stellar Testnet using the **Stellar CLI**.

---

## Gas Consumption Limits

The `admin` contract dispatches calls to the `payment`, `escrow`, and `refund`
contracts in a single Soroban transaction. The table below records the measured
resource consumption of the multi-contract dispatch entry points under the
Soroban test environment (Issue #81).

| Entry point | CPU instructions | Memory bytes | Dominant cost |
| --- | --- | --- | --- |
| `emergency_pause_all` | ~549,000 | ~74,200 | 3 cross-contract `pause_contract` calls |
| `emergency_unpause_all` | ~801,000 | ~113,500 | 3 cross-contract `unpause_contract` calls |
| `get_system_status` | ~440,000 | ~62,700 | 6 read-only cross-contract queries |

### Budget limits

- **CPU instruction limit:** 1,000,000,000 instructions per transaction (current Soroban network limit).
- **Memory limit:** 40,000,000 bytes per transaction.
- The most expensive dispatch (`emergency_unpause_all`) uses roughly **0.08% of the CPU budget** and **0.3% of the memory budget**, leaving ample headroom for the surrounding transaction (signature verification, transaction size, etc.).

> **Note:** Measurements are taken in the Soroban test environment, which
> underestimates CPU and memory relative to the WASM equivalent. Treat the
> numbers above as lower bounds when provisioning mainnet capacity.

### Optimization notes

- The dispatch cost is dominated by the cross-contract calls, which are irreducible: each child contract must be paused/unpaused individually, and each read-only health query is a separate cross-contract call.
- The admin contract reads the three child contract addresses through a single shared helper (`get_contract_addresses`) to avoid duplicating storage reads and error handling across dispatch functions.
- The `reason` string passed to `emergency_pause_all` is cloned once per child contract call, which is unavoidable because each child contract requires its own copy.

---

## Prerequisites

Before beginning, ensure you have the following installed:

1. **Rust & `wasm32` Target**:
   ```bash
   rustup target add wasm32-unknown-unknown
