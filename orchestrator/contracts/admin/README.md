# Admin Contract

Part of the [Cypher GridPay smart contracts](../../README.md) suite on Stellar/Soroban.

## Purpose

The admin contract gates privileged operations across the other contracts. It acts as a central authority that can coordinate emergency actions — such as pausing the payment, escrow, and refund contracts — in a single Soroban call.

## Role & Permission Model

### Admin Address

- The admin is a single `Address` stored in contract instance storage under the `Admin` data key.
- It is set **once** during `initialize(admin, payment_contract, escrow_contract, refund_contract)` and cannot be changed after initialization.
- Calling `initialize` a second time returns `Error::AlreadyInitialized`.
- The admin address must authorize the `initialize` call via Soroban authentication (`require_auth()`).

### Permission Checks

Every privileged function performs two authorization checks:

1. **Authentication** — the caller must pass Soroban's `require_auth()` for the supplied admin address.
2. **Authorization** — the caller's address must match the stored admin address exactly. A mismatch returns `Error::Unauthorized`.

### Privileged Operations

| Function | Description | Admin Required |
|---|---|---|
| `initialize(admin, pauser, payment_contract, escrow_contract, refund_contract)` | Deploys and configures the contract with the admin, the pauser, and the child contract addresses. | Yes (sets the admin) |
| `emergency_pause_all(pauser, reason)` | Atomically pauses the payment, escrow, and refund contracts in one call. | Pauser |
| `emergency_unpause_all(pauser)` | Atomically unpauses all three child contracts. | Pauser |
| `get_coordination_status()` | Read-only. Returns each child's pause state plus an aggregate `consistent` flag. | No |
| `set_payment_contract` / `set_escrow_contract` / `set_refund_contract` | Repoints a managed slot at a new contract. | Yes |

## Atomic Emergency Pause (issue #73)

Pausing three contracts from one call is only safe if it is all-or-nothing.
Without coordination, a failure on the second contract leaves the platform in a
**split state** — for example payment paused while escrow still accepts
operations — which is worse than not pausing at all during an incident.

`emergency_pause_all` and `emergency_unpause_all` therefore run a four-step
sequence:

1. **Load** all three managed addresses. If any is missing the call reverts with
   `NotInitialized` *before* any child is touched.
2. **Pre-flight.** Each child's `get_pause_state().globally_paused` is read. If
   any child is already in the target state the call reverts with
   `ChildAlreadyInTargetState`, so an already-inconsistent set is never made
   worse by a second partial attempt.
3. **Apply.** Each child is invoked through `try_pause_contract` /
   `try_unpause_contract`. Any child error is converted into
   `ChildContractCallFailed`, which reverts the whole Soroban transaction —
   including the writes already made to the children processed earlier in the
   sequence.
4. **Verify.** All three children are read back. If they do not all agree the
   call reverts with `ChildStateVerificationFailed`.

Step 4 also covers the case where a child accepts the call but silently no-ops
(for example a version mismatch that drops the function), which a return-value
check alone would miss.

### Observing Coordination State

`get_coordination_status()` returns a `CoordinationStatus`:

| Field | Meaning |
|---|---|
| `payment_paused` / `escrow_paused` / `refund_paused` | `globally_paused` as reported by each child. `false` when the child cannot be read. |
| `consistent` | `true` only when all three children were readable **and** all three report the same pause state. |
| `children` | One `ContractPauseStatus { contract, address, paused, reachable }` per managed contract, for targeted alerting. |

An unreachable child is never treated as "agreeing" — it is surfaced with
`reachable: false` and forces `consistent: false`.

### Error Codes

| Code | Constant | Description |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` was called more than once. |
| 2 | `NotInitialized` | A privileged function was called before `initialize`, or a managed slot is unset. |
| 3 | `Unauthorized` | The caller's address does not match the stored admin/pauser. |
| 4 | `ChildContractCallFailed` | A child contract refused to pause or unpause. The whole transaction reverts. |
| 5 | `ChildAlreadyInTargetState` | Pre-flight found a child already in the target state; nothing was mutated. |
| 6 | `ChildStateVerificationFailed` | Post-conditions were not met after every child reported success. |

## Security Considerations

- The admin address should be a **multi-sig** or **governance contract** address, never a single private key, to avoid a single point of failure.
- The pauser is a separate credential from the admin so emergency response does not require the long-term admin key. It should follow the same multi-sig guidance.
- Because `emergency_pause_all` halts all child contracts, the pauser key should be treated as a high-value credential and stored securely (e.g., in a hardware wallet or threshold-signing scheme).
- There is no `transfer_admin` or `renounce_admin` function — the admin role is permanent. Review deployment scripts carefully before calling `initialize`.
- `get_coordination_status()` is permissionless on purpose: monitoring must be able to detect a split state even while the platform is mid-incident.


---

## See Also

- [Root README](../../README.md) — architecture overview and workspace setup
- [Payment Contract](../payment/README.md)
- [Escrow Contract](../escrow/README.md)
- [Refund Contract](../refund/README.md)
