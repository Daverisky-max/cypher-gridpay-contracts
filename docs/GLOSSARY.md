# Glossary

## Clawback

An admin-controlled emergency fund-recovery mechanism for the escrow contract. When normal resolution paths (release, dispute, refund) are unavailable — for example, due to fraud, a compliance hold, or an irrecoverable deadlock — a multisig admin can forcibly recover the full escrow balance and transfer it to their own address.

### Lifecycle

Clawback follows a strict three-phase sequence:

1. **Initiate** — An admin calls `initiate_clawback(admin, escrow_id, reason_hash, delay_seconds)`. The `reason_hash` is a 32-byte Keccak-256 (or equivalent) hash of an off-chain document that records the justification. The mandatory `delay_seconds` must be at least 86,400 seconds (24 hours), giving all parties a window to contest or seek remediation before funds move. A unique `request_id` is returned and stored on-chain.

2. **Execute** — After the delay elapses, any admin can call `execute_clawback(admin, request_id)`. The full escrow amount is transferred from the contract to the **admin's address** (not to the original customer), and the escrow status is updated to `Resolved`.

3. **Cancel** — Any admin can call `cancel_clawback(admin, request_id)` at any time before execution to abort the request. Cancellation is final — a cancelled request cannot be re-activated. To retry, a new initiation must be filed.

### Key constraints

- Only registered multisig admins may initiate, execute, or cancel a clawback.
- Only one active (non-executed, non-cancelled) clawback request may exist per escrow at a time. A second initiation for the same escrow while a live request exists returns `AlreadyProcessed`.
- Executing before the delay elapses returns `EscrowError::TimelockNotElapsed`.
- No fees are deducted — the entire locked amount transfers to the admin.

### Error reference

| Error                                | Cause                                                                                                     |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| `BasicError::NotAnAdmin`             | Caller is not in the multisig admin set                                                                   |
| `EscrowError::ClawbackDelayTooShort` | `delay_seconds < 86,400`                                                                                  |
| `EscrowError::NotFound`              | Target escrow does not exist                                                                              |
| `EscrowError::AlreadyProcessed`      | A live clawback request already exists for this escrow (on initiate), or the request was already executed |
| `EscrowError::InvalidStatus`         | Request was already cancelled (on execute)                                                                |
| `EscrowError::TimelockNotElapsed`    | Execution attempted before `execute_after` timestamp                                                      |
| `BasicError::Unauthorized`           | `request_id` not found in storage                                                                         |

## Escrow

A smart contract that locks funds between a customer and a merchant while a transaction is pending, disputed, or held. Escrows track status through a lifecycle of Locked, Released, Disputed, Resolved, and Cancelled.

## Finality Delay

A payment-settlement feature that holds merchant funds for a configurable period after payment completion before releasing them. Payments below a configurable `min_amount_threshold` bypass the delay and settle immediately.

## Hold Period

A mandatory buyer inspection period enforced during escrow creation. The hold period is a duration (in seconds) that specifies how long a buyer has to inspect goods or services after an escrow is locked. The escrow cannot be released or refunded until the hold period elapses. This protects the buyer by giving them time to verify the transaction before merchant release is possible.

The hold period duration is converted to an absolute timestamp (`hold_period_until`) stored on the escrow at creation time. If a release is attempted before `hold_period_until`, the contract returns `EscrowError::HoldPeriodActive`.

## Horizon

The Stellar network's HTTP API used by off-chain services to subscribe to ledger events (such as `EscrowCreated`), enabling real-time notifications without polling.

## Timelock

A security delay enforced on administrative actions in the escrow contract. When an admin queues an action (such as resolving a dispute or forcing a release), the action is subject to a configurable timelock delay before it can be executed. This delay protects against unauthorized or hasty administrative decisions by giving stakeholders time to react.

The timelock duration is configured globally via `TimeLockConfig` (with a default delay and grace period). When an action is queued, the contract calculates `executable_after = now + delay`. If execution is attempted before `executable_after`, the contract returns `EscrowError::TimelockNotElapsed`.

### Semantic Distinction from Hold Period

- **Hold Period**: Protects the buyer during normal escrow lifecycle; applies to buyer-initiated refunds and normal release paths.
- **Timelock**: Protects against admin misbehaviour; applies only to admin-queued actions that bypass normal flow (disputes, forced release).

Together they form a two-tier security model: buyers can inspect (hold period), and admins must wait (timelock) before acting.

## Multisig

Multi-signature governance model used for admin operations across contracts. Actions require a configurable number of approvals from an admin set before they execute, following a proposal-based workflow.

## Reason Code

A type-safe enum (`RefundReasonCode`) that categorises refunds into structured reasons: `ProductDefect`, `NonDelivery`, `DuplicateCharge`, `Unauthorized`, `CustomerRequest`, and `Other`.

## Spend Limits

Per-customer rolling spending caps enforced during payment creation. Each limit specifies a maximum amount within a configurable time window; the counter resets when the window elapses.

## Sub-Account

A labelled partition of an escrow that can be funded and released independently. A parent escrow cannot be fully released until all its sub-accounts have been released.

## Threshold

A configurable minimum value used in multiple contexts: the number of multisig approvals required to execute a proposal, the cumulative weight (in basis points) needed to release a multi-party escrow, or an inactivity period before reputation decay begins.

## WASM

WebAssembly — the compiled bytecode format (`.wasm`) to which Soroban smart contracts are compiled for deployment on the Stellar network, using the `wasm32-unknown-unknown` Rust target.
