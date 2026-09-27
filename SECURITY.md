# Security Policy

This document describes the security vulnerability disclosure process for the Cypher GridPay smart contracts repository.

## Reporting a Vulnerability

If you discover a security vulnerability in this repository, please report it responsibly and do not disclose the issue publicly until a fix has been released.

### Reporting Contact

Email: **security@facilpay.com**

**Response time:** We aim to respond to vulnerability reports within 48 hours.

### What to Include

When reporting a vulnerability, please provide:

1. **Description** — A clear summary of the vulnerability and its potential impact.
2. **Affected Component** — Which contract(s) and function(s) are affected (e.g., payment contract's `complete_payment()`, refund contract's `escalate_to_arbitration()`).
3. **Severity** — Your assessment of severity (Critical, High, Medium, Low).
4. **Steps to Reproduce** — Clear steps or proof-of-concept code demonstrating the issue (without triggering any real damage).
5. **Suggested Fix** — If you have recommendations for remediation, we welcome them.
6. **Contact Information** — Your name, email, and preferred contact method.

## Supported Versions

Security updates are provided for the following versions:

| Version | Status | Support Until |
|---------|--------|----------------|
| Latest main branch | Active | Ongoing |
| Previous tagged release | Limited | 6 months after latest release |
| Older releases | Unsupported | Not applicable |

We recommend always running the latest version to receive security fixes and feature improvements.

## Disclosure Timeline

Once a vulnerability is reported:

1. **Acknowledgment (48 hours)** — We confirm receipt and provide an initial assessment.
2. **Investigation (1–2 weeks)** — Our security team reproduces and analyzes the issue.
3. **Fix Development (1–4 weeks depending on severity)** — A patch is developed and tested.
4. **Pre-release Notification (3–5 days before release)** — We notify downstream projects and major integrators under NDA.
5. **Public Disclosure** — A security advisory is published on GitHub with full details and remediation steps.

We request that researchers refrain from public disclosure until a fix has been released or 90 days have elapsed, whichever comes first.

## Smart Contract Threat Model

### Scope

The Cypher GridPay protocol consists of four Soroban smart contracts deployed on the Stellar network:

- **Payment Contract** (`core/contracts/payment`) — Handles payment creation, completion, refunds, scheduled payments, and fee management.
- **Escrow Contract** (`core/contracts/escrow`) — Manages fund holding, dispute resolution, multi-party escrows, clawback, and vesting.
- **Refund Contract** (`core/contracts/refund`) — Processes refund requests, arbitration, merchant policies, and automated refund rules.
- **Admin Contract** (`orchestrator/contracts/admin`) — Provides administrative control, pause/unpause, and contract cross-registration.

### Threat Categories

#### 1. Reentrancy Attacks
- **Risk**: Malicious token contracts or external calls could re-enter contract functions to manipulate state.
- **Mitigation**: All contracts follow the Checks-Effects-Interactions pattern. Internal state is updated before any external token transfers. No external calls are made to untrusted contracts except through the Stellar token interface.

#### 2. Access Control Bypass
- **Risk**: Unauthorized users could invoke admin-only functions.
- **Mitigation**: All administrative functions require `require_auth()` from authorized admin addresses. Multi-signature requirements are enforced for sensitive operations. Admin succession and threshold checks prevent single-point-of-failure.

#### 3. Front-Running and MEV
- **Risk**: Attackers could front-run payment completion or escrow release transactions.
- **Mitigation**: Payment completion requires admin authorization. Escrow releases are time-locked or require multi-party consensus. Dispute evidence submission deadlines include anti-front-running extensions.

#### 4. Integer Overflow/Underflow
- **Risk**: Arithmetic operations could overflow or underflow, leading to incorrect balances.
- **Mitigation**: Soroban SDK uses checked arithmetic by default. All balance calculations use `i128` with explicit overflow checks. The protocol invariant (Total Locked == Sum(Active Escrows) + Sum(Pending Settlements) + Accumulated Fees) is maintained through careful accounting.

#### 5. Ledger Spam and Rate Limiting
- **Risk**: Public entry points could be spammed to congest the ledger.
- **Mitigation**: Sliding-window rate limits are enforced per caller address. Daily volume caps prevent excessive transaction throughput. Flagged addresses are blocked from creating new payments.

#### 6. Oracle Manipulation
- **Risk**: Price oracle data could be manipulated to affect payment amounts.
- **Mitigation**: Oracle feeds are checked for staleness. Multiple oracle sources can be configured. Circuit breakers halt operations if oracle data is suspicious.

#### 7. Signature Replay
- **Risk**: Off-chain signatures could be replayed across different channels or contracts.
- **Mitigation**: Payment channel signatures include channel ID, sequence number, and contract address. Signatures with sequence numbers less than or equal to the current sequence are rejected.

#### 8. Griefing and Denial of Service
- **Risk**: Attackers could grief other users by submitting frivolous disputes or evidence.
- **Mitigation**: Dispute submission requires staking. Arbitration timeouts prevent indefinite holds. Evidence submission deadlines include automatic extensions for late submissions.

### Contract Security Scope

| Contract | Admin Functions | Public Functions | External Calls |
|----------|----------------|------------------|----------------|
| Payment | `initialize`, `set_fee_config`, `add_admin`, `sweep_fees`, `pause` | `create_payment`, `complete_payment`, `refund_payment`, `get_payment` | Token transfers |
| Escrow | `initialize`, `add_admin`, `pause`, `initiate_clawback` | `create_escrow`, `release_escrow`, `dispute_escrow`, `submit_evidence` | Token transfers |
| Refund | `initialize`, `add_admin`, `set_policy` | `request_refund`, `process_refund`, `escalate_to_arbitration` | Token transfers |
| Admin | `initialize`, `pause`, `unpause` | `get_admin`, `is_paused` | None |

### Bug Bounty Program

We are committed to the security of the Cypher GridPay protocol. A bug bounty program is being established with the following severity tiers:

| Severity | Description | Example |
|----------|-------------|---------|
| Critical | Direct theft of funds or permanent contract compromise | Reentrancy leading to drain of escrowed funds |
| High | Significant fund loss or temporary contract freeze | Access control bypass allowing unauthorized admin actions |
| Medium | Limited fund loss or degraded service | Rate limit bypass enabling ledger spam |
| Low | Minor issues with limited impact | Error message information disclosure |

### PGP Keys

For secure communication of sensitive vulnerability reports, the following PGP keys are available:

| Key ID | Fingerprint | Purpose |
|--------|-------------|---------|
| TBD | TBD | Security vulnerability reports |

*PGP keys will be published here once the security team key infrastructure is finalized.*

### Security Audits

The Cypher GridPay contracts are undergoing professional security audit. Audit reports will be published in the `docs/audits/` directory upon completion.

## Code Review Process

All changes to smart contract code require:
1. At least one admin review approval
2. Passing CI checks (build, test, clippy)
3. No changes to contract storage layout without explicit migration plan
4. Documentation updates for any new public functions
