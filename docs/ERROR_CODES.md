# Centralized Error Catalog

This document catalogs all `#[contracterror]` codes across the Cypher GridPay smart contract suite, covering Payment, Escrow, Refund, and Admin Orchestrator contracts.

---

## 🏛️ Admin Orchestrator Contract (`orchestrator/contracts/admin`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `1` | `AlreadyInitialized` | Contract has already been initialized with component addresses. | Verify configuration; do not re-run initialization. |
| `2` | `NotInitialized` | Contract has not yet been initialized. | Run `initialize` before executing admin operations. |
| `3` | `Unauthorized` | Caller address does not match the configured admin or pauser. | Ensure transaction is signed by the authorized administrator identity. |

---

## 💳 Payment Contract (`core/contracts/payment`)

### Basic & Access Errors (`100`–`199`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `100` | `Unauthorized` | Caller lacks authorization for the invoked entry point. | Sign transaction with merchant, customer, or admin identity as required. |
| `101` | `MetadataTooLarge` | Payload size exceeds maximum byte allowance. | Compact or hash metadata off-chain before submission. |
| `102` | `NotesTooLarge` | Notes string exceeds maximum length. | Truncate notes text. |
| `103` | `InvalidCurrency` | Currency asset code or token address is invalid or unsupported. | Use verified SEP-41 token contracts. |
| `104` | `InvalidBatchSize` | Batch array is empty or exceeds the maximum batch limit. | Split batch into 1 to 50 operations. |
| `105` | `BatchPartialFailure` | One or more items in the batch transfer failed. | Inspect individual transaction statuses and retry failed items. |
| `106` | `RateLimitExceeded` | Calling frequency exceeds rate limit window. | Implement exponential backoff before retrying. |
| `107` | `DailyVolumeExceeded` | Aggregate daily transaction volume ceiling breached. | Wait for rolling 24-hour limit reset or request tier increase. |
| `108` | `AddressFlagged` | Sender or recipient address is flagged by compliance. | Contact compliance operations for identity verification. |
| `109` | `AddressAlreadyFlagged` | Address already exists in compliance flagged list. | No action required. |
| `110` | `AmountExceedsLimit` | Transaction amount exceeds single-payment maximum. | Reduce transaction amount. |
| `111` | `MultiSigNotInitialized` | Multisig governance threshold has not been configured. | Configure multisig admin parameters. |
| `112` | `InsufficientAdmins` | Total admin count is below required threshold. | Register additional administrators. |

### Payment Lifecycle Errors (`200`–`299`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `200` | `NotFound` | Payment ID does not exist in contract storage. | Verify payment identifier. |
| `201` | `InvalidStatus` | Payment status does not allow the requested state transition. | Check payment status (`Pending`, `Completed`, `Cancelled`, `Refunded`). |
| `202` | `AlreadyProcessed` | Payment has already been finalized or refunded. | Do not resubmit completed settlements. |
| `203` | `Expired` | Payment invoice expired before completion. | Create a new payment invoice. |
| `204` | `NotExpired` | Operation can only be performed after expiration. | Wait for expiration timestamp to elapse. |
| `205` | `NoExpiration` | Payment invoice has no expiration timestamp configured. | Ensure payment has an active deadline before checking expiration. |
| `206` | `TransferFailed` | Underling token transfer invocation failed. | Check account balance and token allowance. |
| `207` | `RefundExceedsPayment` | Requested refund exceeds the original paid amount. | Adjust refund amount. |
| `208` | `NotYetDue` | Scheduled settlement delay has not elapsed. | Wait for settlement timelock to mature. |
| `209` | `ScheduledPaymentCancelled` | Scheduled payment was cancelled prior to execution. | Re-schedule payment if necessary. |
| `210` | `MetadataAlreadySet` | Payment metadata is immutable and has already been set. | Do not re-submit metadata. |
| `211` | `MetadataNotFound` | Metadata for payment does not exist. | Submit metadata prior to query. |
| `212` | `HashMismatch` | Provided metadata content hash does not match committed hash. | Verify raw metadata against initial Keccak-256 digest. |
| `213` | `AlreadyFullyPaid` | Payment invoice has already been fully satisfied. | Prevent redundant checkout payments. |

### Subscription & Recurring Billing Errors (`300`–`399`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `300` | `NotFound` | Subscription record not found. | Check subscription ID. |
| `301` | `NotActive` | Subscription is paused, cancelled, or expired. | Reactivate subscription. |
| `302` | `PaymentNotDue` | Billing cycle interval has not arrived. | Wait for the next billing interval. |
| `303` | `MaxRetriesExceeded` | Maximum dunning retry attempts exhausted. | Update customer payment method or cancel subscription. |
| `304` | `Ended` | Subscription reached end of term. | Renew subscription. |
| `305` | `DunningNotFound` | Dunning schedule record not found. | Verify subscription delinquency status. |
| `306` | `NotInDunning` | Subscription is in good standing. | Normal execution path applies. |
| `307` | `RetryNotDue` | Dunning backoff interval has not elapsed. | Wait for scheduled retry time. |
| `308` | `GracePeriodExpired` | Dunning grace period elapsed without payment. | Cancel delinquent subscription and trigger recovery. |
| `309` | `RetryTooEarly` | Attempted retry prior to minimum backoff delay. | Adhere to dunning retry schedule. |
| `310` | `MeteredNotFound` | Metered billing usage record not found. | Record usage before invoicing. |
| `311` | `BillingCapExceeded` | Metered usage exceeds spending ceiling. | Increase spending cap or pause service. |
| `312` | `GroupNotFound` | Shared billing group record not found. | Create group plan. |
| `313` | `AlreadyInGroup` | Account is already an active member of the group. | No action required. |

### Proposal & Feature Errors (`400`–`530`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `400` | `NotFound` | Multisig proposal not found. | Verify proposal ID. |
| `401` | `Expired` | Proposal voting period expired. | Submit new proposal. |
| `402` | `AlreadyExecuted` | Proposal has already been executed. | Do not re-execute. |
| `403` | `ThresholdNotMet` | Approval count does not meet multisig threshold. | Collect required administrator signatures. |
| `404` | `RequiresMultiSig` | Action restricted to multisig proposal execution. | Create a governance proposal. |
| `405` | `InsufficientApprovals` | Not enough approvals collected. | Add signatures. |
| `406` | `ProposalExpired` | Proposal voting deadline passed. | File replacement proposal. |
| `500` | `EscrowMappingNotFound` | Cross-contract escrow mapping not found. | Register escrow link. |
| `501` | `EscrowBridgeFailed` | Cross-contract invocation to escrow failed. | Check escrow contract health and arguments. |
| `502` | `FeeConfigNotFound` | Protocol fee parameters not set. | Configure fee schedule. |
| `503` | `InsufficientFees` | Sweep amount exceeds accumulated protocol fees. | Restrict sweep to `sweep_amount <= accumulated_fees`. |
| `527` | `SpendLimitExceeded` | Customer rolling spend limit exceeded. | Wait for rolling spend window reset. |

---

## 🔒 Escrow Contract (`core/contracts/escrow`)

### Basic & Multisig Errors (`100`–`199`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `100` | `Unauthorized` | Caller lacks authorization. | Provide authorized signer key. |
| `101` | `NotAnAdmin` | Caller is not a registered admin. | Use admin identity. |
| `102` | `AlreadyApproved` | Admin has already approved this proposal. | Wait for remaining admins. |
| `103` | `ContractPaused` | Escrow contract is currently paused by admin orchestrator. | Wait for emergency unpause. |
| `104` | `DuplicateApproval` | Duplicate approval received. | Ignore duplicate. |
| `105` | `MultiSigNotInitialized` | Admin threshold parameters uninitialized. | Initialize contract multisig. |
| `106` | `MigrationNotStarted` | Storage migration has not been initiated. | Initiate migration sequence. |
| `107` | `AlreadyMigrated` | Target record already migrated to new schema. | No action required. |
| `108` | `ParticipantNotFound` | Participant address not found in escrow. | Check participant address. |
| `109` | `InvalidMerkleProof` | Merkle proof verification failed. | Check proof leaf and sibling path. |
| `110` | `RootAlreadyCommitted` | Merkle root already committed for this escrow. | Roots are immutable. |
| `111` | `InvalidBps` | Basis points value exceeds 10,000 (100%). | Ensure basis points sum to <= 10,000. |
| `112` | `InsufficientAdmins` | Number of admins below threshold. | Add admins before proposing. |

### Escrow State Errors (`200`–`299`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `200` | `NotFound` | Escrow ID not found. | Verify escrow ID. |
| `201` | `InvalidStatus` | Escrow status does not permit this action. | Check status (`Locked`, `Released`, `Disputed`, etc.). |
| `202` | `AlreadyProcessed` | Escrow has already been released or refunded. | Do not re-process final escrows. |
| `203` | `ReleaseNotYetAvailable` | Timelock or milestone condition has not been met. | Wait for condition fulfillment. |
| `204` | `TimeoutNotReached` | Expiration or delay timeout has not arrived. | Wait for ledger timestamp to advance. |
| `205` | `ReleaseOnHoldPeriod` | Mandatory hold period active. | Wait for hold period to clear. |
| `206` | `InvalidVestingSchedule` | Vesting parameters (cliff, duration) invalid. | Check vesting configuration. |
| `207` | `CliffPeriodNotPassed` | Vesting cliff has not elapsed. | Wait for cliff date. |
| `208` | `MilestoneAlreadyReleased` | Milestone tranche already paid out. | Check next milestone. |
| `209` | `EscrowNotExpired` | Escrow expiration date has not arrived. | Expiration action requires elapsed deadline. |
| `210` | `EscrowAlreadyExpired` | Escrow already passed expiration deadline. | Trigger expired escrow refund. |
| `211` | `ExpiryBeforeRelease` | Expiration timestamp cannot precede release timestamp. | Adjust timestamps. |
| `212` | `ClawbackDelayTooShort` | Clawback delay parameter must be >= 86,400s (24 hours). | Increase delay to minimum 24 hours. |
| `213` | `InvalidDisputeWindow` | Dispute window duration configuration invalid. | Provide valid window. |
| `214` | `DisputeWindowPassed` | Dispute filing deadline has elapsed. | Dispute cannot be opened after window closes. |
| `215` | `EvidenceDeadlinePassed` | Counter-evidence submission window closed. | Check if deadline extension applies. |
| `216` | `DisputeAlreadyOpen` | An active dispute is already open for this escrow. | Submit evidence to existing dispute. |
| `217` | `DisputeNotFound` | No dispute record exists for this escrow. | File dispute before requesting resolution. |
| `218` | `ResolutionThresholdNotMet` | Arbitrator consensus threshold not reached. | Collect required arbitrator rulings. |

### Actions & Secondary Errors (`300`–`399`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `300` | `NotReady` | Precondition delay not satisfied. | Wait for execution window. |
| `301` | `NotDisputed` | Operation requires escrow in `Disputed` status. | Open dispute first. |
| `302` | `ObserverAlreadyAdded` | Observer address already registered. | Do not add duplicate observer. |
| `303` | `ObserverNotFound` | Target observer address not registered. | Register observer before lookup. |
| `304` | `AccelerationLimitExceeded` | Vesting acceleration exceeds maximum allowed. | Cap acceleration rate. |
| `305` | `TransferNotAllowed` | Beneficiary transfer restricted on this escrow. | Check transfer restrictions. |
| `306` | `SameBeneficiary` | New beneficiary address identical to current. | Provide distinct beneficiary. |
| `307` | `ConditionAlreadyEvaluated` | Milestone condition has already evaluated. | Proceed to next milestone. |
| `308` | `StaleThresholdNotConfigured` | Stale timeout value unconfigured. | Set threshold before invocation. |
| `309` | `SwapConfigNotFound` | Asset exchange routing parameters not found. | Configure DEX router. |
| `310` | `SwapOutputBelowMinimum` | Slippage tolerance exceeded minimum output amount. | Adjust slippage or wait for liquidity. |
| `311` | `SwapAlreadyExecuted` | Token conversion already executed. | Do not re-run swap. |

---

## 🔄 Refund & Arbitration Contract (`core/contracts/refund`)

| Error Code | Name | Description | Recommended Client Action |
| :---: | :--- | :--- | :--- |
| `1` | `NotFound` | Refund request record not found. | Verify refund request ID. |
| `2` | `Unauthorized` | Caller lacks permission to act on refund. | Sign with customer or merchant key. |
| `3` | `InvalidStatus` | Refund in invalid status for this operation. | Verify refund status (`Requested`, `Approved`, `Denied`, `Appealed`). |
| `4` | `AlreadyProcessed` | Refund already processed and finalized. | Do not re-submit finalized refund. |
| `5` | `AmountExceedsPayment` | Refund amount exceeds original purchase total. | Reduce refund amount. |
| `6` | `CooldownActive` | Mandatory cooling-off period active between attempts. | Wait for cooldown interval to pass. |
| `7` | `AppealWindowExpired` | Appeal filing window elapsed after denial. | Appeals must be filed within active appeal window. |
| `8` | `InvalidArbitrator` | Arbitrator address is not registered or active. | Use active registered arbitrator. |
| `9` | `DisputeAlreadyResolved` | Dispute appeal already resolved. | Final rulings cannot be overturned. |
| `10` | `VoucherExpired` | Store credit voucher validity period elapsed. | Request voucher reissuance if merchant allows. |
| `34` | `ArbitratorNotFound` | Arbitrator record not found in registry. | Register arbitrator first. |
| `35` | `InvalidScoreThreshold` | Reputation score threshold configuration invalid. | Provide valid score range. |
| `36` | `AutoRefundTriggerNotFound` | Auto-refund trigger condition missing. | Configure trigger rule. |
| `37` | `DuplicateAutoRefundTrigger` | Auto-refund trigger already exists. | Use existing trigger ID. |
| `38` | `AddressFlaggedForFraud` | Account address flagged for abusive refund requests. | Contact platform security. |
| `40` | `FraudSignalNotFound` | Fraud metric signal not found. | Ingest telemetry signal first. |
| `41` | `HookNotFound` | Webhook subscription record not found. | Verify hook ID. |
| `42` | `MaxHooksPerEventReached` | Maximum registered webhooks limit reached. | Remove obsolete hooks before adding new ones. |
