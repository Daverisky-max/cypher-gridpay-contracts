# Security Policy

This document describes the security vulnerability disclosure process for the Cypher GridPay smart contracts repository.

## Reporting a Vulnerability

If you discover a security vulnerability in this repository, please report it responsibly and do not disclose the issue publicly until a fix has been released.

### Reporting Contact

Email: **security@facilpay.com**

**Response time:** We aim to respond to vulnerability reports within 48 hours.

### Encrypted Reports (PGP)

Reports may be encrypted with PGP to protect researcher contact details and
proof-of-concept material in transit.

**Status: key not yet published — do not encrypt until this section names a
fingerprint.** The maintainers must publish their key below; until then, use the
plaintext `security@facilpay.com` channel above.

| Field | Value |
|---|---|
| Key ID / fingerprint | `_TO BE PUBLISHED BY MAINTAINERS_` |
| Key type / algorithm | `_TO BE PUBLISHED BY MAINTAINERS_` |
| Expiry | `_TO BE PUBLISHED BY MAINTAINERS_` |
| Uploaded to | `_TO BE PUBLISHED BY MAINTAINERS_` |

> **Maintainer action required before merging:** replace every
> `_TO BE PUBLISHED BY MAINTAINERS_` placeholder above with the real key
> parameters, and publish the armored public key at a stable, well-known URL
> with a detached signature. A placeholder that is never filled in means
> researchers will encrypt to nothing; that is worse than offering no PGP option.

To verify the key once published, fetch it and check that the fingerprint the
key reports matches the fingerprint published in this table **before** encrypting
to it:

```bash
gpg --keyserver keyserver.ubuntu.com --recv-keys <KEY_ID>
gpg --fingerprint <KEY_ID>   # compare against the table above
```

Send the encrypted report and the key you used to `security@facilpay.com`.
Decrypt the body and follow the submission checklist below. We will acknowledge
receipt within 48 hours, as for plaintext reports.



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
4. **Pre-release Notification (3–5 days before release)** — We notify downstream projects (API repo, SDK repo) of the fix.
5. **Public Disclosure (on release)** — The fix is released publicly; we issue a security advisory and credit the researcher.

### Expedited Timeline for Critical Issues

Critical vulnerabilities (e.g., fund loss, contract compromise) are prioritized:

- **Fix Target:** 1 week
- **Release Target:** 2 weeks from initial report
- **Pre-release notification:** 5 days before release

## Security Scope of the Contracts

The following are the deployed contracts and the state each one owns. Reports
should name the specific contract and function.

| Contract | Path | Owns | Privilege model |
|---|---|---|---|
| Payment | `core/contracts/payment` | Payment records, fees, channels, schedule | Public; admin for settlement/refund |
| Escrow | `core/contracts/escrow` | Escrow records, disputes, evidence, collateral | Public; admin for release |
| Refund | `core/contracts/refund` | Refund requests, policy, arbitration, vouchers | Public; merchant + arbitrator roles |
| Admin (orchestrator) | `orchestrator/contracts/admin` | Admin set, pause state, cross-contract wiring | Multi-sig, highly privileged |

**Explicitly not in scope of this repository:**

- The API, indexer, or any off-chain service.
- The Stellar network, Soroban host, or `soroban-sdk` itself (report upstream).
- Third-party token contracts deployed by someone other than the protocol.
- Test fixtures, testnet deployment keys, and example configuration.
- Client SDKs and generated bindings (separate repositories).


## Threat Model

This section describes the adversary we design against, the trust boundaries in
the system, and the invariants each contract relies on. It is intended to let
researchers reason about whether a finding is a genuine vulnerability in scope.

### System Trust Boundaries

```
  Customer / Merchant  ──►  Token Contract (SEP-41 SAC)   ──►  Core Contracts
   (untrusted)              (external, trusted)              (payment, escrow,
                                  ▲                             refund)
                                  │
                            Admin Orchestrator
                        (privileged, multi-sig)
```

| Component | Trust level | Notes |
|---|---|---|
| `payment`, `escrow`, `refund` (core) | Untrusted caller | Every entry point is reachable by any Stellar account; authorization is enforced in-contract. |
| `admin` (orchestrator) | Highly privileged | Can pause, rotate admins, sweep fees. Compromise here is assumed catastrophic. |
| Token contract | External / trusted | Standard SEP-41 asset. We do not control its admin or supply. |
| Off-chain services (API, indexer) | Out of scope | Cannot influence on-chain state. |
| Ledger / Soroban host | Trusted | Out of scope; consensus and host bugs are not in our control. |

### Adversary Capabilities

We assume an adversary who can:

1. **Call any public contract function at any time**, from any account, including
   accounts they created.
2. **Submit transactions in any order**, and interleave their own transactions
   with a victim's (mempool observation / front-running).
3. **Control all accounts they own** and can fund them arbitrarily on testnet.
4. **Collude with a malicious merchant** — i.e. merchant-side misbehaviour is in
   scope; only *honest* merchant behaviour is assumed.
5. **Re-read any public on-chain state** and any event emitted so far.
6. **Re-submit or replay previously submitted signed payloads** (relevant to
   off-chain signed payment channels).

We explicitly **do not** assume: private-key compromise of a user or merchant
account, a compromised RPC/horizon node, or a malicious Stellar host.

### Invariants Relied Upon

| # | Invariant | Enforced by |
|---|---|---|
| I-1 | Balance conservation: no tokens are created or destroyed by the protocol. | Accounting paths in `payment`/`escrow`/`refund`. Formal statement: [`docs/ECONOMIC_INVARIANTS.md`](docs/ECONOMIC_INVARIANTS.md). |
| I-2 | Caller authorization is checked before any state mutation for every privileged function. | `require_auth` / `require_admin` guards. |
| I-3 | Internal accounting is settled before any external token transfer (checks-effects-interactions), so a re-entrant call cannot observe a partially updated balance. | Settlement functions. |
| I-4 | Escrowed or unfinalized merchant funds are never reachable by fee-sweep or admin withdrawal paths. | `sweep_fees` is bounded by `accumulated_fees`. |
| I-5 | A signed off-chain balance update is accepted at most once and only for the channel, sequence number, and contract it was issued for. | Signature verification in payment channels. |
| I-6 | An escrowed balance can be released to exactly one of: merchant, customer, or arbitrator — never more than once. | Escrow state machine transitions. |

A violation of any of I-1 … I-6 is a **critical** finding.

### Abuse Cases We Explicitly Consider In-Scope

- Griefing or denial-of-service against a legitimate counterparty, e.g. spam
  creating payments or refund requests to exhaust shared resources.
- Last-second dispute evidence submission that leaves a counterparty no time to
  respond.
- Fee-sweep or withdrawal paths that reach beyond accumulated protocol fees.
- Replay of an off-chain signed balance update against a different channel,
  sequence number, or contract address.
- Missing or incorrect `require_auth` on administrative or privileged functions.

## What We Consider a Vulnerability

### In Scope

- Unauthorized fund transfer or lockup
- Contract state corruption or bypass of access controls
- Integer overflow/underflow leading to incorrect balances
- Cross-contract call failures that leave escrow in an unsafe state
- Signature/authentication bypass
- Reentrancy or state machine violations
- Cryptographic weaknesses
- Event emission failures that break off-chain indexers

### Out of Scope

- Issues in documentation or comments (report via pull request instead)
- Speculative issues without proof-of-concept
- Performance issues that don't affect correctness
- Vulnerabilities in dependent libraries (report to the library maintainers)
- Social engineering or phishing attacks

## Bug Bounty

At this time, we do not operate a formal bug bounty program. However, we deeply appreciate security researchers who help us improve the safety of our contracts. Researchers who responsibly disclose vulnerabilities will be:

- **Credited** in our security advisory and this repository
- **Acknowledged** in release notes
- **Considered for future bug bounty programs**

## Security Best Practices for Integrators

If you are integrating these contracts into your application:

1. **Keep Updated** — Subscribe to releases and apply security patches promptly.
2. **Audit Dependent Contracts** — These contracts rely on external escrow and token contracts; ensure those are audited and trusted.
3. **Monitor Events** — Use the documented Soroban events to verify contract behavior off-chain.
4. **Test Edge Cases** — Particularly around refund limits, multi-sig governance, and arbitration timeouts.
5. **Rate Limiting** — Enable the built-in rate limiting and fraud detection features.
6. **Access Controls** — Use multi-sig governance for sensitive operations like admin upgrades.

## Public Disclosure

Once a fix is released, we will:

1. Publish a security advisory in this repository
2. Tag the release with a security indicator
3. Document the issue in the CHANGELOG.md
4. Credit the researcher (unless they request anonymity)

## Contact & Questions

For security-related inquiries other than vulnerability reports, please contact:

**security@facilpay.com**

For general questions or feature requests, see the root [README.md](README.md) for community links.

---

**Last Updated:** 2026-09-27

For the most up-to-date security information, visit the [FacilPay security page](https://facilpay.com/security).
