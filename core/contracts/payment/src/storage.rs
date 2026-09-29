// Storage keys and enums for the payment contract
// This module organizes all DataKey variants and related constants

use soroban_sdk::{contracttype, Address, BytesN};

use crate::types::Currency;

// Configuration keys for contract-wide settings
#[derive(Clone)]
#[contracttype]
pub enum ConfigKey {
    Admin,
    MultiSigConfig,
    FeeConfig,
    RateLimitConfig,
    DunningConfig,
    LoyaltyConfig,
    RiskFeeConfig,
    FinalityConfig,
    FeeRebateConfig,
    TierThresholds,
    LargePaymentThreshold,
    GlobalMerchantCount,
    PauseStateKey,
    MinSplitAmount,
    SchemaVersion,
    AllowedTokens,
    MaxForwardDepth,
}

// Payment-specific storage keys
#[derive(Clone)]
#[contracttype]
pub enum PaymentKey {
    Data(u64),
    Counter,
    Metadata(u64),
    Memo(u64),
    MemoVersion(u64),
    Tag(u64),
    Invoice(u64),
    InvoiceCounter,
    InvoicePaymentId(u64),
    PartialPaymentCounter(u64),
    OutstandingBalance(u64),
    PendingSettlement(u64),
    AccumulatedFees,
    LargePaymentCounter,
    Discount(u64),
}

pub const MAX_MEMO_VERSIONS: u32 = 10;

// Subscription-specific storage keys
#[derive(Clone)]
#[contracttype]
pub enum SubscriptionKey {
    Data(u64),
    Counter,
    Metered(u64),
    MeteredCounter,
    Group(u64),
    GroupCounter,
    GroupMembership(u64),
}

// Feature-specific storage keys (payment channels, splits, analytics, etc.)
#[derive(Clone)]
#[contracttype]
pub enum FeatureKey {
    PaymentAnalytics,
    PlatformAnalyticsDaily(u64),
    PaymentForwardConfig(Address),
    OracleRateConfig(Currency),
    ConversionRate(Currency),
    MerchantRateLimit(Address),
    CustomerLoyaltyBalance(Address),
    CustomerSpendLimit(Address),
    PaymentChannel(u64),
    PaymentChannelCounter,
    SplitConfig(u64),
    SweepRecipient,
    SweepCounter,
    SweepHistory(u64),
    RouteOptions(Address, Address),
}

// Root data key enum - organizes all storage into categories
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config(ConfigKey),
    Payment(PaymentKey),
    Subscription(SubscriptionKey),
    Feature(FeatureKey),
    Customer(CustomerDataKey),
    Merchant(MerchantDataKey),
    State(StateDataKey),
}

// Customer-specific data keys
#[derive(Clone)]
#[contracttype]
pub enum CustomerDataKey {
    Payments(Address, u64),
    PaymentCount(Address),
    Subscriptions(Address, u64),
    SubscriptionCount(Address),
    Analytics(Address),
    RateLimit(Address),
    FlagReason(Address),
    Allowlist(Address),
    FeeWaiver(Address),
    MerchantVolume(Address, Address),
    MerchantList(Address, u64),
    MerchantCount(Address),
    MonthlyVolume(Address, u64),
    HourCount(Address, u32),
}

// Merchant-specific data keys
#[derive(Clone)]
#[contracttype]
pub enum MerchantDataKey {
    Payments(Address, u64),
    PaymentCount(Address),
    Subscriptions(Address, u64),
    SubscriptionCount(Address),
    Analytics(Address),
    FeeRecord(Address),
    AnalyticsBucket(Address, u64),
    GlobalList(u64),
    PayoutSchedule(Address),
    RebateAccrual(Address),
    PendingSettlementCount(Address),
    PendingSettlementIndex(Address, u64),
    VerificationLevel(Address),
    VerificationTierLimit(crate::types::MerchantVerificationLevel),
    MerchantPaymentsPage(Address, u64),
    MerchantPaused(Address),
    MerchantActiveSubscriptions(Address, u64),
    MerchantActiveSubscriptionCount(Address),
    ActiveSubscriptionIndex(u64),
}

// State and proposal data keys
#[derive(Clone)]
#[contracttype]
pub enum StateDataKey {
    DunningState(u64),
    EscrowedPayment(u64),
    EscrowedPaymentDispute(u64),
    ConditionalPayment(u64),
    ScheduledPayment(u64),
    AdminProposal(soroban_sdk::String),
    LargePaymentProposal(u64),
    PauseHistoryEntry(u64),
    PauseHistoryCount,
    // Auto-escrow
    AutoEscrowRule(Address),
    AutoEscrowTriggered(u64),
    PartialPaymentRecord(u64, u32), // payment_id, installment_number
    SettlementFinalized(u64),
    ScheduledPaymentCounter,
}
