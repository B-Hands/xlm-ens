use core::fmt;
use core::future::Future;
use core::pin::Pin;

use crate::errors::{SdkError, SigningError};

pub const DEFAULT_FEE_CURRENCY: &str = "XLM";

pub type SignerFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Vec<u8>, SigningError>> + Send + 'a>>;

/// Abstracts the signing boundary for the xlm-ns SDK.
///
/// Implement this trait to control how transaction envelopes are signed:
/// - Use [`KeypairSigner`] when the secret key is available in-process.
/// - Use [`ExternalSigner`] to delegate signing to a closure, wallet hook,
///   or hardware device without exposing private key material to the SDK.
///
/// The SDK calls `sign_transaction` exactly once per submitted transaction,
/// after simulation and fee attachment, and before submission.
pub trait Signer: Send + Sync {
    /// Sign the XDR transaction envelope bytes and return the signed envelope bytes.
    fn sign_transaction(&self, tx_envelope_xdr: &[u8]) -> SignerFuture<'_>;

    /// Return the public key this signer controls, for source account resolution.
    fn public_key(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct Keypair {
    secret_key: String,
    public_key: String,
}

impl Keypair {
    pub fn from_secret(secret_key: &str) -> Result<Self, SigningError> {
        let trimmed = secret_key.trim();
        if trimmed.is_empty() {
            return Err(SigningError::InvalidKey {
                reason: "secret key must not be empty".to_string(),
            });
        }
        if !trimmed.starts_with('S') || trimmed.len() < 2 {
            return Err(SigningError::InvalidKey {
                reason: "secret key must start with 'S'".to_string(),
            });
        }

        let strkey =
            stellar_strkey::Strkey::from_string(trimmed).map_err(|_| SigningError::InvalidKey {
                reason: "failed to decode secret key strkey encoding".to_string(),
            })?;
        let seed = match strkey {
            stellar_strkey::Strkey::PrivateKeyEd25519(s) => s,
            _ => {
                return Err(SigningError::InvalidKey {
                    reason: "expected an ed25519 private key".to_string(),
                });
            }
        };
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed.0);
        let verifying_key = signing_key.verifying_key();
        let public_key = stellar_strkey::ed25519::PublicKey(verifying_key.to_bytes()).to_string();

        Ok(Self {
            secret_key: trimmed.to_string(),
            public_key,
        })
    }
}

pub struct KeypairSigner {
    keypair: Keypair,
}

impl KeypairSigner {
    pub fn new(secret_key: &str) -> Result<Self, SdkError> {
        let keypair =
            Keypair::from_secret(secret_key).map_err(|source| SdkError::SigningFailed {
                operation: "constructing keypair signer",
                source,
            })?;
        Ok(Self { keypair })
    }
}

impl Signer for KeypairSigner {
    fn sign_transaction(&self, tx_envelope_xdr: &[u8]) -> SignerFuture<'_> {
        let _ = &self.keypair.secret_key;
        let signed = tx_envelope_xdr.to_vec();
        Box::pin(async move { Ok(signed) })
    }

    fn public_key(&self) -> &str {
        &self.keypair.public_key
    }
}

pub struct ExternalSigner<F>
where
    F: Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync,
{
    sign_fn: F,
    pubkey: String,
}

impl<F> ExternalSigner<F>
where
    F: Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync,
{
    pub fn new(pubkey: impl Into<String>, sign_fn: F) -> Self {
        Self {
            sign_fn,
            pubkey: pubkey.into(),
        }
    }
}

impl<F> Signer for ExternalSigner<F>
where
    F: Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync,
{
    fn sign_transaction(&self, tx_envelope_xdr: &[u8]) -> SignerFuture<'_> {
        let result = (self.sign_fn)(tx_envelope_xdr)
            .map_err(|reason| SigningError::ExternalFailure { reason });
        Box::pin(async move { result })
    }

    fn public_key(&self) -> &str {
        &self.pubkey
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationRequest {
    pub label: String,
    pub owner: String,
    pub duration_years: u32,
    pub signer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeeBreakdown {
    pub base_fee: u64,
    pub premium_fee: u64,
    pub network_fee: u64,
}

impl FeeBreakdown {
    pub fn total(&self) -> u64 {
        self.base_fee
            .saturating_add(self.premium_fee)
            .saturating_add(self.network_fee)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationQuote {
    pub label: String,
    pub duration_years: u32,
    pub fee_breakdown: FeeBreakdown,
    pub total_fee: u64,
    pub fee_currency: String,
    pub expires_at: u64,
    pub grace_period_ends_at: u64,
    pub quoted_at: u64,
    pub contract_id: Option<String>,
}

/// The registrar's current lifecycle classification for a label.
///
/// `Unavailable` means the label has no current registration record, while
/// `Claimable` means a previous registration has passed its grace period. Both
/// states can be registered by a new owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationStatus {
    Unavailable,
    Active,
    GracePeriod,
    Claimable,
    Reserved,
}

/// A registration preview combining lifecycle, ownership, and pricing data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailabilityResult {
    /// Whether the label can be registered by a new owner now.
    pub available: bool,
    /// The registrar lifecycle status used to determine availability.
    pub status: RegistrationStatus,
    /// A complete registration quote when the label is registerable.
    pub quote: Option<RegistrationQuote>,
    /// The current registrant for active or grace-period names.
    pub current_owner: Option<String>,
    /// The current registration expiry for active or grace-period names.
    pub expires_at: Option<u64>,
}

impl RegistrationQuote {
    /// Backwards-friendly accessor: the headline fee a caller should pay.
    pub fn fee(&self) -> u64 {
        self.total_fee
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewalRequest {
    pub name: String,
    pub additional_years: u32,
    pub signer: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmissionStatus {
    Simulated,
    Submitted,
    Confirmed,
    Failed,
}

impl fmt::Display for SubmissionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simulated => f.write_str("simulated"),
            Self::Submitted => f.write_str("submitted"),
            Self::Confirmed => f.write_str("confirmed"),
            Self::Failed => f.write_str("failed"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionSubmission {
    pub tx_hash: String,
    pub status: SubmissionStatus,
    pub ledger: Option<u32>,
    pub submitted_at: u64,
    pub contract_id: Option<String>,
    pub network_passphrase: Option<String>,
    pub signer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationReceipt {
    pub name: String,
    pub owner: String,
    pub duration_years: u32,
    pub expires_at: u64,
    pub fee_paid: u64,
    pub submission: TransactionSubmission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterResult {
    pub name: String,
    pub owner: String,
    pub tx_hash: String,
    pub ledger_sequence: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewalReceipt {
    pub name: String,
    pub additional_years: u32,
    pub new_expiry: u64,
    pub fee_paid: u64,
    pub submission: TransactionSubmission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewResult {
    pub name: String,
    pub new_expiry_ledger: u32,
    pub tx_hash: String,
    pub ledger_sequence: u32,
}

/// Retained for backwards compatibility with callers that only need
/// the raw renewal outcome.
pub type RenewalResult = RenewalReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionResult {
    pub name: String,
    pub address: Option<String>,
    pub resolver: Option<String>,
    pub expires_at: Option<u64>,
    pub is_wildcard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioPage {
    pub items: Vec<ResolutionResult>,
    pub next_cursor: Option<usize>,
    pub total: usize,
}

/// Why a single name inside a batch failed to resolve.
///
/// A batch never fails as a whole because of one bad name — the failure is
/// attached to that name's [`BatchResult`] and every other name still returns
/// its resolution. See [`XlmNsClient::batch_resolve`].
///
/// [`XlmNsClient::batch_resolve`]: crate::client::XlmNsClient::batch_resolve
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchResolveError {
    /// The name failed client-side validation and was never sent to the
    /// contract (empty, whitespace-only, or otherwise malformed).
    InvalidName { reason: String },
    /// The resolver returned no record for this name.
    NotFound,
    /// A record exists but the registration lapsed past its grace period, so
    /// the resolver no longer treats it as active.
    Expired { expired_at: u64 },
    /// A record exists but carries no address for the default chain.
    NoAddress,
    /// The chunk containing this name could not be queried, even after the
    /// configured retries. Other chunks are unaffected.
    Rpc { reason: String },
}

impl fmt::Display for BatchResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { reason } => write!(f, "invalid name: {reason}"),
            Self::NotFound => f.write_str("no resolver record found"),
            Self::Expired { expired_at } => {
                write!(f, "registration expired at unix timestamp {expired_at}")
            }
            Self::NoAddress => f.write_str("record has no address for the default chain"),
            Self::Rpc { reason } => write!(f, "rpc failure: {reason}"),
        }
    }
}

impl std::error::Error for BatchResolveError {}

/// The outcome of resolving one name within a batch.
///
/// Exactly one of [`address`](Self::address) and [`error`](Self::error) is
/// populated: a successful entry carries the resolved address and TTL, a
/// failed entry carries the per-name reason. Use [`is_ok`](Self::is_ok) to
/// branch without matching on both fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchResult {
    /// The name as supplied by the caller, so results can be correlated with
    /// the input even though ordering is already preserved.
    pub name: String,
    /// The resolved address, or `None` when [`error`](Self::error) is set.
    pub address: Option<String>,
    /// Record time-to-live in seconds, for callers that cache resolutions.
    pub ttl_seconds: Option<u64>,
    /// Unix timestamp at which the registration expires.
    pub expires_at: Option<u64>,
    /// Why this name failed, or `None` when resolution succeeded.
    pub error: Option<BatchResolveError>,
}

impl BatchResult {
    /// Build a successful entry.
    pub fn success(
        name: impl Into<String>,
        address: impl Into<String>,
        ttl_seconds: Option<u64>,
        expires_at: Option<u64>,
    ) -> Self {
        Self {
            name: name.into(),
            address: Some(address.into()),
            ttl_seconds,
            expires_at,
            error: None,
        }
    }

    /// Build a failed entry.
    pub fn failure(name: impl Into<String>, error: BatchResolveError) -> Self {
        Self {
            name: name.into(),
            address: None,
            ttl_seconds: None,
            expires_at: None,
            error: Some(error),
        }
    }

    /// `true` when this name resolved successfully.
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    /// `true` when this name failed to resolve.
    pub fn is_err(&self) -> bool {
        self.error.is_some()
    }

    /// The resolved address, or the per-name error.
    pub fn as_result(&self) -> Result<&str, &BatchResolveError> {
        match (&self.address, &self.error) {
            (Some(address), None) => Ok(address.as_str()),
            (_, Some(error)) => Err(error),
            // Unreachable via the constructors above; treat a record with
            // neither an address nor an error as a missing address.
            (None, None) => Err(&BatchResolveError::NoAddress),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReverseResolution {
    pub address: String,
    pub primary_name: Option<String>,
    pub resolver: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRecord {
    pub name: String,
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRecordUpdate {
    pub name: String,
    pub key: String,
    pub value: Option<String>,
    pub signer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRecordsUpdate {
    pub name: String,
    pub records: std::collections::HashMap<String, Option<String>>,
    pub signer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferRequest {
    pub name: String,
    pub new_owner: String,
    pub signer: Option<String>,
}

// Subdomain types
#[derive(Debug, Clone)]
pub struct RegisterParentRequest {
    pub parent: String,
    pub owner: String,
}

#[derive(Debug, Clone)]
pub struct AddControllerRequest {
    pub parent: String,
    pub controller: String,
}

#[derive(Debug, Clone)]
pub struct CreateSubdomainRequest {
    pub label: String,
    pub parent: String,
    pub owner: String,
}

#[derive(Debug, Clone)]
pub struct TransferSubdomainRequest {
    pub fqdn: String,
    pub new_owner: String,
}

#[derive(Debug, Clone)]
pub struct ParentDomain {
    pub owner: String,
    pub controllers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SubdomainRecord {
    pub parent: String,
    pub owner: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subdomain {
    pub label: String,
    pub owner: String,
}

// Bridge types
#[derive(Debug, Clone)]
pub struct RegisterChainRequest {
    pub chain: String,
}

#[derive(Debug, Clone)]
pub struct BuildMessageRequest {
    pub name: String,
    pub chain: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeRoute {
    pub destination_chain: String,
    pub destination_resolver: String,
    pub gateway: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NftRecord {
    pub token_id: String,
    pub owner: String,
    pub metadata_uri: Option<String>,
}

/// Cumulative fee and operation metrics returned by the registrar's read APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrarMetrics {
    pub treasury_balance: u64,
    pub total_registrations: u64,
    pub total_renewals: u64,
}

// Domain Models
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameRecord {
    pub owner: String,
    pub registered_at: u64,
    pub expires_at: u64,
    pub grace_period_ends_at: u64,
    pub resolver: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuctionState {
    pub highest_bid: i128,
    pub end_time: u64,
}

// Contract types for RPC calls
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegistryEntry {
    pub name: String,
    pub owner: String,
    pub resolver: Option<String>,
    pub target_address: Option<String>,
    pub metadata_uri: Option<String>,
    pub ttl_seconds: u64,
    pub registered_at: u64,
    pub expires_at: u64,
    pub grace_period_ends_at: u64,
    pub transfer_count: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResolutionRecord {
    pub owner: String,
    pub address: String,
    pub text_records: std::collections::HashMap<String, String>,
    pub updated_at: u64,
    pub expires_at: u64,
    pub is_wildcard: bool,
}

// Auction types
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuctionInfo {
    pub name: String,
    pub owner: String,
    pub reserve_price: u64,
    pub highest_bid: u64,
    pub highest_bidder: Option<String>,
    /// Number of bids accepted by the auction so far.
    pub bid_count: u32,
    pub ends_at: u64,
    pub status: AuctionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum AuctionStatus {
    Active,
    Ended,
    Settled,
}

impl fmt::Display for AuctionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => f.write_str("active"),
            Self::Ended => f.write_str("ended"),
            Self::Settled => f.write_str("settled"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuctionCreateRequest {
    pub name: String,
    pub asset: String,
    pub treasury: String,
    pub reserve_price: u64,
    pub duration_seconds: u64,
    pub signer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BidRequest {
    pub name: String,
    pub amount: u64,
    pub signer: Option<String>,
}

/// Typed output from a pre-flight simulation of a write operation.
///
/// Call `simulate_register()` or `simulate_renew()` to inspect fees,
/// auth requirements, and preflight errors before committing a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationResult {
    /// Estimated fee in stroops that the operation will consume.
    pub fee_estimate: u64,
    /// Addresses whose authorization is required for the transaction.
    pub auth_addresses: Vec<String>,
    /// Human-readable error message if simulation detected a contract error.
    pub error: Option<String>,
    /// `true` when simulation found no errors and the transaction can be submitted.
    pub success: bool,
}
