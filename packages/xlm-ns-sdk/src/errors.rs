use core::fmt;

/// Contract-specific error codes returned by Soroban on-chain contracts.
///
/// Each variant is prefixed with the contract name so that overlapping numeric
/// codes (every contract starts from 1) can be unambiguously represented.
///
/// Use [`decode_error`] to convert a `(contract, code)` pair into a variant,
/// or [`decode_error_generic`] when the originating contract is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractErrorCode {
    // ── Registry (codes 1-11) ────────────────────────────────────────────────
    /// The name is already registered and cannot be registered again.
    RegistryAlreadyRegistered,
    /// The name was not found in the registry.
    RegistryNotFound,
    /// The name has expired but is still in its grace period and not yet claimable.
    RegistryNotYetClaimable,
    /// The name is not currently active (e.g. expired).
    RegistryNotActive,
    /// The caller is not authorized to perform this registry action.
    RegistryUnauthorized,
    /// The provided metadata URI exceeds the maximum allowed length.
    RegistryMetadataTooLong,
    /// The input failed registry validation (name format, boundaries, etc.).
    RegistryValidation,
    /// The expiry timestamp is not in the future.
    RegistryInvalidExpiry,
    /// The grace period end is before the expiry timestamp.
    RegistryInvalidGracePeriod,
    /// The registry contract upgrade failed.
    RegistryUpgradeFailed,
    /// The name is locked for dispute resolution.
    RegistryLocked,

    // ── Registrar (codes 1-13) ───────────────────────────────────────────────
    /// The fee paid is less than the required registration fee.
    RegistrarInsufficientFee,
    /// The name was not found in the registrar.
    RegistrarNotFound,
    /// The name is not in a renewable state.
    RegistrarNotRenewable,
    /// The name is already registered.
    RegistrarAlreadyRegistered,
    /// The name label is reserved and cannot be publicly registered.
    RegistrarReserved,
    /// The caller is not authorized for this registrar action.
    RegistrarUnauthorized,
    /// The input failed registrar validation.
    RegistrarValidation,
    /// The name is claimable (expired past grace period) rather than renewable.
    RegistrarRegistrationClaimable,
    /// The registrar contract is not yet initialized.
    RegistrarNotInitialized,
    /// The registrar contract is already initialized.
    RegistrarAlreadyInitialized,
    /// The rate limit for registrations has been exceeded.
    RegistrarRateLimitExceeded,
    /// The registrar contract upgrade failed.
    RegistrarUpgradeFailed,
    /// The registration quote has expired; request a fresh quote.
    RegistrarQuoteExpired,

    // ── Resolver (codes 1-10) ────────────────────────────────────────────────
    /// The input failed resolver validation.
    ResolverValidation,
    /// The requested resolver record was not found.
    ResolverRecordNotFound,
    /// The caller is not authorized for this resolver action.
    ResolverUnauthorized,
    /// The name already has the maximum number of text records.
    ResolverTooManyTextRecords,
    /// The resolver contract is not yet initialized.
    ResolverNotInitialized,
    /// The text record value exceeds the maximum allowed length.
    ResolverTextRecordValueTooLong,
    /// The chain identifier is not supported by this resolver.
    ResolverInvalidChain,
    /// The text record key is invalid or not normalized.
    ResolverInvalidKey,
    /// The batch request contains too many operations.
    ResolverBatchTooLarge,
    /// The resolver contract upgrade failed.
    ResolverUpgradeFailed,

    // ── Subdomain (codes 1-7) ────────────────────────────────────────────────
    /// The input failed subdomain validation.
    SubdomainValidation,
    /// The parent domain was not found in the registry.
    SubdomainParentNotFound,
    /// The subdomain already exists under this parent.
    SubdomainAlreadyExists,
    /// The subdomain was not found.
    SubdomainNotFound,
    /// The caller is not authorized for this subdomain action.
    SubdomainUnauthorized,
    /// The subdomain contract upgrade failed.
    SubdomainUpgradeFailed,
    /// The requested subdomain path exceeds the maximum nesting depth.
    SubdomainDepthLimitExceeded,

    // ── Auction (codes 1-10) ─────────────────────────────────────────────────
    /// The input failed auction validation.
    AuctionValidation,
    /// An auction for this name already exists.
    AuctionAlreadyExists,
    /// No auction was found for this name.
    AuctionNotFound,
    /// The auction is already closed and cannot accept bids.
    AuctionClosed,
    /// The auction has not started yet.
    AuctionNotStarted,
    /// The auction has not ended yet; settlement is not possible.
    AuctionNotEnded,
    /// The auction was already settled.
    AuctionAlreadySettled,
    /// The bid amount is below the reserve price or minimum increment.
    AuctionInvalidBid,
    /// The auction contract upgrade failed.
    AuctionUpgradeFailed,
    /// A reentrancy guard blocked this auction operation.
    AuctionReentrancyDetected,

    // ── Bridge (codes 1-6) ───────────────────────────────────────────────────
    /// The input failed bridge validation.
    BridgeValidation,
    /// The target chain is not supported by the bridge.
    BridgeUnsupportedChain,
    /// The bridge contract upgrade failed.
    BridgeUpgradeFailed,
    /// The caller is not authorized for this bridge action.
    BridgeUnauthorized,
    /// The requested chain or route was not found.
    BridgeNotFound,
    /// The chain or route already exists in the bridge.
    BridgeAlreadyExists,

    // ── NFT (codes 1-5) ──────────────────────────────────────────────────────
    /// The token has already been minted.
    NftAlreadyMinted,
    /// The token was not found.
    NftNotFound,
    /// The caller is not authorized to modify or inspect this token.
    NftUnauthorized,
    /// The NFT contract upgrade failed.
    NftUpgradeFailed,
    /// The NFT contract is not yet initialized.
    NftNotInitialized,

    // ── Generic / legacy ─────────────────────────────────────────────────────
    /// Legacy alias — name not found (generic, contract unknown).
    NameNotFound,
    /// Legacy alias — caller is not the owner (generic, contract unknown).
    NotOwner,
    /// Legacy alias — the record or name has expired (generic, contract unknown).
    Expired,
    /// Legacy alias — the label is invalid (generic, contract unknown).
    InvalidLabel,
    /// The contract returned an unrecognised error code.
    Other(u32),
}

#[derive(Debug)]
pub enum SdkError {
    InvalidRequest(String),
    Transport(String),
    Ingestion(String),
    ContractError(ContractErrorCode),
    /// The network passphrase returned by the RPC server does not
    /// match the passphrase configured in the SDK client.
    NetworkPassphraseMismatch {
        configured: String,
        rpc_reported: String,
    },
    /// The network passphrase embedded in a transaction does not
    /// match the passphrase configured in the SDK client.
    TransactionPassphraseMismatch {
        configured: String,
        in_transaction: String,
    },
    ContractInvocationFailed {
        operation: &'static str,
        reason: String,
        tx_hash: Option<String>,
    },
    SimulationFailed {
        operation: &'static str,
        reason: String,
    },
    InsufficientFee {
        operation: &'static str,
        required: i64,
        available: i64,
    },
    TransactionTimeout {
        operation: &'static str,
        ledger_submitted: u32,
    },
    SigningFailed {
        operation: &'static str,
        source: SigningError,
    },
    RateLimitExceeded(RateLimitError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitError {
    pub retries: u32,
    pub total_wait_ms: u64,
}

impl fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rate limit exceeded after {} retries (waited {}ms)",
            self.retries, self.total_wait_ms
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningError {
    Rejected { reason: String },
    InvalidKey { reason: String },
    ExternalFailure { reason: String },
    MalformedEnvelope { reason: String },
}

impl fmt::Display for ContractErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            // Registry
            Self::RegistryAlreadyRegistered => "registry: name is already registered",
            Self::RegistryNotFound => "registry: name was not found",
            Self::RegistryNotYetClaimable => "registry: name is expired but still in grace period",
            Self::RegistryNotActive => "registry: name is not currently active",
            Self::RegistryUnauthorized => "registry: caller is not authorized for this name",
            Self::RegistryMetadataTooLong => "registry: metadata URI exceeds the allowed length",
            Self::RegistryValidation => "registry: input failed validation",
            Self::RegistryInvalidExpiry => "registry: expires_at must be in the future",
            Self::RegistryInvalidGracePeriod => {
                "registry: grace_period_ends_at must be >= expires_at"
            }
            Self::RegistryUpgradeFailed => "registry: contract upgrade failed",
            Self::RegistryLocked => "registry: name is locked for dispute resolution",
            // Registrar
            Self::RegistrarInsufficientFee => "registrar: fee paid is below the required amount",
            Self::RegistrarNotFound => "registrar: name was not found",
            Self::RegistrarNotRenewable => "registrar: name is not in a renewable state",
            Self::RegistrarAlreadyRegistered => "registrar: name is already registered",
            Self::RegistrarReserved => "registrar: name label is reserved",
            Self::RegistrarUnauthorized => "registrar: caller is not authorized",
            Self::RegistrarValidation => "registrar: input failed validation",
            Self::RegistrarRegistrationClaimable => "registrar: name is claimable, not renewable",
            Self::RegistrarNotInitialized => "registrar: contract is not initialized",
            Self::RegistrarAlreadyInitialized => "registrar: contract is already initialized",
            Self::RegistrarRateLimitExceeded => "registrar: registration rate limit exceeded",
            Self::RegistrarUpgradeFailed => "registrar: contract upgrade failed",
            Self::RegistrarQuoteExpired => "registrar: registration quote has expired",
            // Resolver
            Self::ResolverValidation => "resolver: input failed validation",
            Self::ResolverRecordNotFound => "resolver: record was not found",
            Self::ResolverUnauthorized => "resolver: caller is not authorized",
            Self::ResolverTooManyTextRecords => "resolver: name has too many text records",
            Self::ResolverNotInitialized => "resolver: contract is not initialized",
            Self::ResolverTextRecordValueTooLong => "resolver: text record value is too long",
            Self::ResolverInvalidChain => "resolver: chain identifier is not supported",
            Self::ResolverInvalidKey => "resolver: text record key is invalid",
            Self::ResolverBatchTooLarge => "resolver: batch request contains too many operations",
            Self::ResolverUpgradeFailed => "resolver: contract upgrade failed",
            // Subdomain
            Self::SubdomainValidation => "subdomain: input failed validation",
            Self::SubdomainParentNotFound => "subdomain: parent domain was not found",
            Self::SubdomainAlreadyExists => "subdomain: subdomain already exists",
            Self::SubdomainNotFound => "subdomain: subdomain was not found",
            Self::SubdomainUnauthorized => "subdomain: caller is not authorized",
            Self::SubdomainUpgradeFailed => "subdomain: contract upgrade failed",
            Self::SubdomainDepthLimitExceeded => "subdomain: path exceeds maximum nesting depth",
            // Auction
            Self::AuctionValidation => "auction: input failed validation",
            Self::AuctionAlreadyExists => "auction: an auction already exists for this name",
            Self::AuctionNotFound => "auction: no auction found for this name",
            Self::AuctionClosed => "auction: auction is closed and cannot accept bids",
            Self::AuctionNotStarted => "auction: auction has not started yet",
            Self::AuctionNotEnded => "auction: auction has not ended yet",
            Self::AuctionAlreadySettled => "auction: auction was already settled",
            Self::AuctionInvalidBid => {
                "auction: bid is below the reserve price or minimum increment"
            }
            Self::AuctionUpgradeFailed => "auction: contract upgrade failed",
            Self::AuctionReentrancyDetected => "auction: reentrancy guard blocked this operation",
            // Bridge
            Self::BridgeValidation => "bridge: input failed validation",
            Self::BridgeUnsupportedChain => "bridge: target chain is not supported",
            Self::BridgeUpgradeFailed => "bridge: contract upgrade failed",
            Self::BridgeUnauthorized => "bridge: caller is not authorized",
            Self::BridgeNotFound => "bridge: chain or route was not found",
            Self::BridgeAlreadyExists => "bridge: chain or route already exists",
            // NFT
            Self::NftAlreadyMinted => "nft: token has already been minted",
            Self::NftNotFound => "nft: token was not found",
            Self::NftUnauthorized => "nft: caller is not authorized",
            Self::NftUpgradeFailed => "nft: contract upgrade failed",
            Self::NftNotInitialized => "nft: contract is not initialized",
            // Generic / legacy
            Self::NameNotFound => "name was not found",
            Self::NotOwner => "caller is not the owner",
            Self::Expired => "name or record has expired",
            Self::InvalidLabel => "label is not a valid XLM name",
            Self::Other(code) => return write!(f, "unknown contract error code {code}"),
        };
        f.write_str(msg)
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => write!(f, "invalid request: {message}"),
            Self::Transport(message) => write!(f, "transport error: {message}"),
            Self::Ingestion(message) => write!(f, "ingestion error: {message}"),
            Self::ContractError(code) => write!(f, "contract error: {code}"),
            Self::NetworkPassphraseMismatch {
                configured,
                rpc_reported,
            } => write!(
                f,
                "network passphrase mismatch: configured={configured:?}, rpc_reported={rpc_reported:?}"
            ),
            Self::TransactionPassphraseMismatch {
                configured,
                in_transaction,
            } => write!(
                f,
                "transaction passphrase mismatch: configured={configured:?}, in_transaction={in_transaction:?}"
            ),
            Self::ContractInvocationFailed {
                operation,
                reason,
                tx_hash,
            } => {
                if let Some(tx_hash) = tx_hash {
                    write!(f, "{operation} failed: {reason} (tx: {tx_hash})")
                } else {
                    write!(f, "{operation} failed: {reason}")
                }
            }
            Self::SimulationFailed { operation, reason } => {
                write!(f, "{operation} simulation failed: {reason}")
            }
            Self::InsufficientFee {
                operation,
                required,
                available,
            } => write!(
                f,
                "{operation} has insufficient fee: required {required}, available {available}"
            ),
            Self::TransactionTimeout {
                operation,
                ledger_submitted,
            } => write!(
                f,
                "{operation} timed out after submission at ledger {ledger_submitted}"
            ),
            Self::SigningFailed { operation, source } => {
                write!(f, "{operation} signing failed: {source}")
            }
            Self::RateLimitExceeded(err) => write!(f, "{err}"),
        }
    }
}

impl fmt::Display for SigningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { reason } => write!(f, "signing rejected: {reason}"),
            Self::InvalidKey { reason } => write!(f, "invalid signing key: {reason}"),
            Self::ExternalFailure { reason } => write!(f, "external signer failed: {reason}"),
            Self::MalformedEnvelope { reason } => {
                write!(f, "malformed transaction envelope: {reason}")
            }
        }
    }
}

impl std::error::Error for SdkError {}
impl std::error::Error for SigningError {}

/// Decode a contract error code into a typed [`ContractErrorCode`] given the
/// name of the originating contract.
///
/// Pass the short lowercase contract identifier as `contract`:
/// `"registry"`, `"registrar"`, `"resolver"`, `"subdomain"`,
/// `"auction"`, `"bridge"`, or `"nft"`.
///
/// Unknown `(contract, code)` pairs return [`ContractErrorCode::Other`].
pub fn decode_error(contract: &str, code: u32) -> ContractErrorCode {
    match (contract, code) {
        // Registry
        ("registry", 1) => ContractErrorCode::RegistryAlreadyRegistered,
        ("registry", 2) => ContractErrorCode::RegistryNotFound,
        ("registry", 3) => ContractErrorCode::RegistryNotYetClaimable,
        ("registry", 4) => ContractErrorCode::RegistryNotActive,
        ("registry", 5) => ContractErrorCode::RegistryUnauthorized,
        ("registry", 6) => ContractErrorCode::RegistryMetadataTooLong,
        ("registry", 7) => ContractErrorCode::RegistryValidation,
        ("registry", 8) => ContractErrorCode::RegistryInvalidExpiry,
        ("registry", 9) => ContractErrorCode::RegistryInvalidGracePeriod,
        ("registry", 10) => ContractErrorCode::RegistryUpgradeFailed,
        ("registry", 11) => ContractErrorCode::RegistryLocked,
        // Registrar
        ("registrar", 1) => ContractErrorCode::RegistrarInsufficientFee,
        ("registrar", 2) => ContractErrorCode::RegistrarNotFound,
        ("registrar", 3) => ContractErrorCode::RegistrarNotRenewable,
        ("registrar", 4) => ContractErrorCode::RegistrarAlreadyRegistered,
        ("registrar", 5) => ContractErrorCode::RegistrarReserved,
        ("registrar", 6) => ContractErrorCode::RegistrarUnauthorized,
        ("registrar", 7) => ContractErrorCode::RegistrarValidation,
        ("registrar", 8) => ContractErrorCode::RegistrarRegistrationClaimable,
        ("registrar", 9) => ContractErrorCode::RegistrarNotInitialized,
        ("registrar", 10) => ContractErrorCode::RegistrarAlreadyInitialized,
        ("registrar", 11) => ContractErrorCode::RegistrarRateLimitExceeded,
        ("registrar", 12) => ContractErrorCode::RegistrarUpgradeFailed,
        ("registrar", 13) => ContractErrorCode::RegistrarQuoteExpired,
        // Resolver
        ("resolver", 1) => ContractErrorCode::ResolverValidation,
        ("resolver", 2) => ContractErrorCode::ResolverRecordNotFound,
        ("resolver", 3) => ContractErrorCode::ResolverUnauthorized,
        ("resolver", 4) => ContractErrorCode::ResolverTooManyTextRecords,
        ("resolver", 5) => ContractErrorCode::ResolverNotInitialized,
        ("resolver", 6) => ContractErrorCode::ResolverTextRecordValueTooLong,
        ("resolver", 7) => ContractErrorCode::ResolverInvalidChain,
        ("resolver", 8) => ContractErrorCode::ResolverInvalidKey,
        ("resolver", 9) => ContractErrorCode::ResolverBatchTooLarge,
        ("resolver", 10) => ContractErrorCode::ResolverUpgradeFailed,
        // Subdomain
        ("subdomain", 1) => ContractErrorCode::SubdomainValidation,
        ("subdomain", 2) => ContractErrorCode::SubdomainParentNotFound,
        ("subdomain", 3) => ContractErrorCode::SubdomainAlreadyExists,
        ("subdomain", 4) => ContractErrorCode::SubdomainNotFound,
        ("subdomain", 5) => ContractErrorCode::SubdomainUnauthorized,
        ("subdomain", 6) => ContractErrorCode::SubdomainUpgradeFailed,
        ("subdomain", 7) => ContractErrorCode::SubdomainDepthLimitExceeded,
        // Auction
        ("auction", 1) => ContractErrorCode::AuctionValidation,
        ("auction", 2) => ContractErrorCode::AuctionAlreadyExists,
        ("auction", 3) => ContractErrorCode::AuctionNotFound,
        ("auction", 4) => ContractErrorCode::AuctionClosed,
        ("auction", 5) => ContractErrorCode::AuctionNotStarted,
        ("auction", 6) => ContractErrorCode::AuctionNotEnded,
        ("auction", 7) => ContractErrorCode::AuctionAlreadySettled,
        ("auction", 8) => ContractErrorCode::AuctionInvalidBid,
        ("auction", 9) => ContractErrorCode::AuctionUpgradeFailed,
        ("auction", 10) => ContractErrorCode::AuctionReentrancyDetected,
        // Bridge
        ("bridge", 1) => ContractErrorCode::BridgeValidation,
        ("bridge", 2) => ContractErrorCode::BridgeUnsupportedChain,
        ("bridge", 3) => ContractErrorCode::BridgeUpgradeFailed,
        ("bridge", 4) => ContractErrorCode::BridgeUnauthorized,
        ("bridge", 5) => ContractErrorCode::BridgeNotFound,
        ("bridge", 6) => ContractErrorCode::BridgeAlreadyExists,
        // NFT
        ("nft", 1) => ContractErrorCode::NftAlreadyMinted,
        ("nft", 2) => ContractErrorCode::NftNotFound,
        ("nft", 3) => ContractErrorCode::NftUnauthorized,
        ("nft", 4) => ContractErrorCode::NftUpgradeFailed,
        ("nft", 5) => ContractErrorCode::NftNotInitialized,
        // Anything unrecognised
        (_, code) => ContractErrorCode::Other(code),
    }
}

/// Decode a raw contract error code without knowing which contract produced it.
///
/// This is a backward-compatible fallback that uses the original 4-code
/// mapping.  Prefer [`decode_error`] whenever the originating contract is
/// known.
pub fn decode_error_generic(code: u32) -> ContractErrorCode {
    match code {
        1 => ContractErrorCode::NameNotFound,
        2 => ContractErrorCode::NotOwner,
        3 => ContractErrorCode::Expired,
        4 => ContractErrorCode::InvalidLabel,
        _ => ContractErrorCode::Other(code),
    }
}

/// Returns `true` when an RPC call may be retried with exponential backoff.
pub fn is_retryable(err: &SdkError) -> bool {
    match err {
        SdkError::TransactionTimeout { .. } => true,
        SdkError::Transport(message) => transport_is_retryable(message),
        SdkError::InvalidRequest(_)
        | SdkError::Ingestion(_)
        | SdkError::ContractError(_)
        | SdkError::NetworkPassphraseMismatch { .. }
        | SdkError::TransactionPassphraseMismatch { .. }
        | SdkError::ContractInvocationFailed { .. }
        | SdkError::SimulationFailed { .. }
        | SdkError::InsufficientFee { .. }
        | SdkError::SigningFailed { .. }
        | SdkError::RateLimitExceeded { .. } => false,
    }
}

fn transport_is_retryable(message: &str) -> bool {
    let msg = message.to_ascii_lowercase();

    // Configuration / parsing failures are permanent.
    if msg.contains("invalid rpc url")
        || msg.contains("json decoding error")
        || msg.contains("invalid response from server")
    {
        return false;
    }

    // HTTP rate limiting and server-side transient failures.
    if msg.contains("429")
        || msg.contains("too many requests")
        || msg.contains("rate limit")
        || msg.contains("500")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
        || msg.contains("service unavailable")
    {
        return true;
    }

    // Network-level blips.
    for marker in [
        "timeout",
        "timed out",
        "connection refused",
        "connection reset",
        "connection closed",
        "broken pipe",
        "network unreachable",
        "dns error",
        "temporary failure",
        "failed to send",
        "error sending request",
        "error trying to connect",
    ] {
        if msg.contains(marker) {
            return true;
        }
    }

    // Unclassified transport errors default to retryable.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_retryable_only_for_transient_errors() {
        assert!(is_retryable(&SdkError::Transport("timeout".into())));
        assert!(is_retryable(&SdkError::Transport(
            "http error: status 503 service unavailable".into()
        )));
        assert!(is_retryable(&SdkError::Transport(
            "too many requests (429)".into()
        )));
        assert!(is_retryable(&SdkError::Transport(
            "connection refused".into()
        )));
        assert!(!is_retryable(&SdkError::Transport(
            "invalid rpc url: bad://".into()
        )));
        assert!(is_retryable(&SdkError::TransactionTimeout {
            operation: "register",
            ledger_submitted: 42,
        }));
        assert!(!is_retryable(&SdkError::InvalidRequest("bad input".into())));
        assert!(!is_retryable(&SdkError::ContractError(
            ContractErrorCode::NameNotFound
        )));
        assert!(!is_retryable(&SdkError::NetworkPassphraseMismatch {
            configured: "a".into(),
            rpc_reported: "b".into(),
        }));
        assert!(!is_retryable(&SdkError::SimulationFailed {
            operation: "register",
            reason: "revert".into(),
        }));
        assert!(!is_retryable(&SdkError::ContractInvocationFailed {
            operation: "register",
            reason: "revert".into(),
            tx_hash: None,
        }));
    }

    // ── decode_error: Registry ────────────────────────────────────────────────

    #[test]
    fn decode_registry_errors() {
        assert_eq!(
            decode_error("registry", 1),
            ContractErrorCode::RegistryAlreadyRegistered
        );
        assert_eq!(
            decode_error("registry", 2),
            ContractErrorCode::RegistryNotFound
        );
        assert_eq!(
            decode_error("registry", 3),
            ContractErrorCode::RegistryNotYetClaimable
        );
        assert_eq!(
            decode_error("registry", 4),
            ContractErrorCode::RegistryNotActive
        );
        assert_eq!(
            decode_error("registry", 5),
            ContractErrorCode::RegistryUnauthorized
        );
        assert_eq!(
            decode_error("registry", 6),
            ContractErrorCode::RegistryMetadataTooLong
        );
        assert_eq!(
            decode_error("registry", 7),
            ContractErrorCode::RegistryValidation
        );
        assert_eq!(
            decode_error("registry", 8),
            ContractErrorCode::RegistryInvalidExpiry
        );
        assert_eq!(
            decode_error("registry", 9),
            ContractErrorCode::RegistryInvalidGracePeriod
        );
        assert_eq!(
            decode_error("registry", 10),
            ContractErrorCode::RegistryUpgradeFailed
        );
        assert_eq!(
            decode_error("registry", 11),
            ContractErrorCode::RegistryLocked
        );
    }

    // ── decode_error: Registrar ───────────────────────────────────────────────

    #[test]
    fn decode_registrar_errors() {
        assert_eq!(
            decode_error("registrar", 1),
            ContractErrorCode::RegistrarInsufficientFee
        );
        assert_eq!(
            decode_error("registrar", 2),
            ContractErrorCode::RegistrarNotFound
        );
        assert_eq!(
            decode_error("registrar", 3),
            ContractErrorCode::RegistrarNotRenewable
        );
        assert_eq!(
            decode_error("registrar", 4),
            ContractErrorCode::RegistrarAlreadyRegistered
        );
        assert_eq!(
            decode_error("registrar", 5),
            ContractErrorCode::RegistrarReserved
        );
        assert_eq!(
            decode_error("registrar", 6),
            ContractErrorCode::RegistrarUnauthorized
        );
        assert_eq!(
            decode_error("registrar", 7),
            ContractErrorCode::RegistrarValidation
        );
        assert_eq!(
            decode_error("registrar", 8),
            ContractErrorCode::RegistrarRegistrationClaimable
        );
        assert_eq!(
            decode_error("registrar", 9),
            ContractErrorCode::RegistrarNotInitialized
        );
        assert_eq!(
            decode_error("registrar", 10),
            ContractErrorCode::RegistrarAlreadyInitialized
        );
        assert_eq!(
            decode_error("registrar", 11),
            ContractErrorCode::RegistrarRateLimitExceeded
        );
        assert_eq!(
            decode_error("registrar", 12),
            ContractErrorCode::RegistrarUpgradeFailed
        );
        assert_eq!(
            decode_error("registrar", 13),
            ContractErrorCode::RegistrarQuoteExpired
        );
    }

    // ── decode_error: Resolver ────────────────────────────────────────────────

    #[test]
    fn decode_resolver_errors() {
        assert_eq!(
            decode_error("resolver", 1),
            ContractErrorCode::ResolverValidation
        );
        assert_eq!(
            decode_error("resolver", 2),
            ContractErrorCode::ResolverRecordNotFound
        );
        assert_eq!(
            decode_error("resolver", 3),
            ContractErrorCode::ResolverUnauthorized
        );
        assert_eq!(
            decode_error("resolver", 4),
            ContractErrorCode::ResolverTooManyTextRecords
        );
        assert_eq!(
            decode_error("resolver", 5),
            ContractErrorCode::ResolverNotInitialized
        );
        assert_eq!(
            decode_error("resolver", 6),
            ContractErrorCode::ResolverTextRecordValueTooLong
        );
        assert_eq!(
            decode_error("resolver", 7),
            ContractErrorCode::ResolverInvalidChain
        );
        assert_eq!(
            decode_error("resolver", 8),
            ContractErrorCode::ResolverInvalidKey
        );
        assert_eq!(
            decode_error("resolver", 9),
            ContractErrorCode::ResolverBatchTooLarge
        );
        assert_eq!(
            decode_error("resolver", 10),
            ContractErrorCode::ResolverUpgradeFailed
        );
    }

    // ── decode_error: Subdomain ───────────────────────────────────────────────

    #[test]
    fn decode_subdomain_errors() {
        assert_eq!(
            decode_error("subdomain", 1),
            ContractErrorCode::SubdomainValidation
        );
        assert_eq!(
            decode_error("subdomain", 2),
            ContractErrorCode::SubdomainParentNotFound
        );
        assert_eq!(
            decode_error("subdomain", 3),
            ContractErrorCode::SubdomainAlreadyExists
        );
        assert_eq!(
            decode_error("subdomain", 4),
            ContractErrorCode::SubdomainNotFound
        );
        assert_eq!(
            decode_error("subdomain", 5),
            ContractErrorCode::SubdomainUnauthorized
        );
        assert_eq!(
            decode_error("subdomain", 6),
            ContractErrorCode::SubdomainUpgradeFailed
        );
        assert_eq!(
            decode_error("subdomain", 7),
            ContractErrorCode::SubdomainDepthLimitExceeded
        );
    }

    // ── decode_error: Auction ─────────────────────────────────────────────────

    #[test]
    fn decode_auction_errors() {
        assert_eq!(
            decode_error("auction", 1),
            ContractErrorCode::AuctionValidation
        );
        assert_eq!(
            decode_error("auction", 2),
            ContractErrorCode::AuctionAlreadyExists
        );
        assert_eq!(
            decode_error("auction", 3),
            ContractErrorCode::AuctionNotFound
        );
        assert_eq!(decode_error("auction", 4), ContractErrorCode::AuctionClosed);
        assert_eq!(
            decode_error("auction", 5),
            ContractErrorCode::AuctionNotStarted
        );
        assert_eq!(
            decode_error("auction", 6),
            ContractErrorCode::AuctionNotEnded
        );
        assert_eq!(
            decode_error("auction", 7),
            ContractErrorCode::AuctionAlreadySettled
        );
        assert_eq!(
            decode_error("auction", 8),
            ContractErrorCode::AuctionInvalidBid
        );
        assert_eq!(
            decode_error("auction", 9),
            ContractErrorCode::AuctionUpgradeFailed
        );
        assert_eq!(
            decode_error("auction", 10),
            ContractErrorCode::AuctionReentrancyDetected
        );
    }

    // ── decode_error: Bridge ──────────────────────────────────────────────────

    #[test]
    fn decode_bridge_errors() {
        assert_eq!(
            decode_error("bridge", 1),
            ContractErrorCode::BridgeValidation
        );
        assert_eq!(
            decode_error("bridge", 2),
            ContractErrorCode::BridgeUnsupportedChain
        );
        assert_eq!(
            decode_error("bridge", 3),
            ContractErrorCode::BridgeUpgradeFailed
        );
        assert_eq!(
            decode_error("bridge", 4),
            ContractErrorCode::BridgeUnauthorized
        );
        assert_eq!(decode_error("bridge", 5), ContractErrorCode::BridgeNotFound);
        assert_eq!(
            decode_error("bridge", 6),
            ContractErrorCode::BridgeAlreadyExists
        );
    }

    // ── decode_error: NFT ─────────────────────────────────────────────────────

    #[test]
    fn decode_nft_errors() {
        assert_eq!(decode_error("nft", 1), ContractErrorCode::NftAlreadyMinted);
        assert_eq!(decode_error("nft", 2), ContractErrorCode::NftNotFound);
        assert_eq!(decode_error("nft", 3), ContractErrorCode::NftUnauthorized);
        assert_eq!(decode_error("nft", 4), ContractErrorCode::NftUpgradeFailed);
        assert_eq!(decode_error("nft", 5), ContractErrorCode::NftNotInitialized);
    }

    // ── decode_error: fallbacks ───────────────────────────────────────────────

    #[test]
    fn decode_error_unknown_contract_returns_other() {
        assert_eq!(decode_error("unknown", 1), ContractErrorCode::Other(1));
        assert_eq!(decode_error("registry", 99), ContractErrorCode::Other(99));
        assert_eq!(decode_error("registrar", 42), ContractErrorCode::Other(42));
    }

    #[test]
    fn decode_error_generic_backward_compat() {
        assert_eq!(decode_error_generic(1), ContractErrorCode::NameNotFound);
        assert_eq!(decode_error_generic(2), ContractErrorCode::NotOwner);
        assert_eq!(decode_error_generic(3), ContractErrorCode::Expired);
        assert_eq!(decode_error_generic(4), ContractErrorCode::InvalidLabel);
        assert_eq!(decode_error_generic(99), ContractErrorCode::Other(99));
    }

    // ── Display ───────────────────────────────────────────────────────────────

    #[test]
    fn display_shows_descriptive_messages() {
        assert_eq!(
            ContractErrorCode::RegistryAlreadyRegistered.to_string(),
            "registry: name is already registered"
        );
        assert_eq!(
            ContractErrorCode::RegistrarInsufficientFee.to_string(),
            "registrar: fee paid is below the required amount"
        );
        assert_eq!(
            ContractErrorCode::RegistrarQuoteExpired.to_string(),
            "registrar: registration quote has expired"
        );
        assert_eq!(
            ContractErrorCode::ResolverRecordNotFound.to_string(),
            "resolver: record was not found"
        );
        assert_eq!(
            ContractErrorCode::AuctionInvalidBid.to_string(),
            "auction: bid is below the reserve price or minimum increment"
        );
        assert_eq!(
            ContractErrorCode::BridgeUnsupportedChain.to_string(),
            "bridge: target chain is not supported"
        );
        assert_eq!(
            ContractErrorCode::NftNotInitialized.to_string(),
            "nft: contract is not initialized"
        );
        assert_eq!(
            ContractErrorCode::Other(7).to_string(),
            "unknown contract error code 7"
        );
    }

    #[test]
    fn sdk_error_display_uses_descriptive_contract_message() {
        let err = SdkError::ContractError(ContractErrorCode::RegistrarInsufficientFee);
        assert_eq!(
            err.to_string(),
            "contract error: registrar: fee paid is below the required amount"
        );
    }
}
