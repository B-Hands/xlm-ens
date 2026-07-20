# Contract Error Code Reference

This document is the single source of truth for every error code emitted by the
seven xlm-ens Soroban contracts.  When a transaction fails, the Soroban RPC
returns a numeric code with no surrounding context.  Use this reference to
translate that number into an actionable explanation.

---

## How Soroban Contract Errors Work

Each contract defines an `#[contracterror]` enum whose variants are encoded as
`u32` integers on-chain.  When a transaction reverts, the RPC response contains:

```json
{
  "error": "HostError: Error(Contract, #1)"
}
```

The `#1` is the raw error code.  Because every contract starts numbering from
`1`, the same number can mean different things in different contracts.  You
**must** know which contract generated the error before you can interpret it.

The SDK's [`decode_error(contract, code)`][sdk-errors] function translates a
`(contract_name, code)` pair into a typed `ContractErrorCode` variant with a
human-readable description.

[sdk-errors]: ../packages/xlm-ns-sdk/src/errors.rs

---

## Quick Reference — 10 Most Common Errors

| # | Contract | Code | Error | One-line Fix |
|---|----------|------|-------|--------------|
| 1 | registrar | 1 | `RegistrarInsufficientFee` | Fund the account and re-quote |
| 2 | registrar | 4 | `RegistrarAlreadyRegistered` | Use `xlm-ns whois` to find the owner |
| 3 | registrar | 13 | `RegistrarQuoteExpired` | Run `xlm-ns quote` for a fresh quote |
| 4 | registry | 1 | `RegistryAlreadyRegistered` | Name is taken; check with `xlm-ns whois` |
| 5 | registry | 2 | `RegistryNotFound` | Check spelling or use `xlm-ns resolve` |
| 6 | registry | 3 | `RegistryNotYetClaimable` | Wait for the grace period to end |
| 7 | resolver | 2 | `ResolverRecordNotFound` | Set a resolver record first |
| 8 | registrar | 6 | `RegistrarUnauthorized` | Switch to the owner signer |
| 9 | subdomain | 2 | `SubdomainParentNotFound` | Register the parent domain first |
| 10 | auction | 8 | `AuctionInvalidBid` | Increase bid above reserve price |

---

## Error Code Overlap Note

Every contract starts its error numbering from `1`.  Code `1` in the **registry**
(`RegistryAlreadyRegistered`) means something entirely different from code `1` in
the **registrar** (`RegistrarInsufficientFee`).  Always identify the originating
contract before interpreting a numeric code.

The SDK's `decode_error(contract, code)` handles this automatically:

```rust
use xlm_ns_sdk::errors::{decode_error, ContractErrorCode};

let typed = decode_error("registrar", 1);
// → ContractErrorCode::RegistrarInsufficientFee
println!("{}", typed);
// → "registrar: fee paid is below the required amount"
```

---

## Registry Contract

**Source:** `contracts/registry/src/lib.rs`

The registry is the central ownership ledger.  It maps `.xlm` names to owners,
expiry times, and metadata.

| Error Name | Code | Description | Common Cause | Resolution |
|------------|------|-------------|--------------|------------|
| `AlreadyRegistered` | 1 | The name is already owned by another account | Attempting to register a taken name | Use `xlm-ns whois <name>` to find the current owner |
| `NotFound` | 2 | The name does not exist in the registry | Typo, or name was never registered | Check the spelling; use `xlm-ns resolve <name>` |
| `NotYetClaimable` | 3 | The name has expired but is still in its grace period | Trying to claim an expired name too early | Wait until the grace period ends; check with `xlm-ns whois` |
| `NotActive` | 4 | The name is expired and not in an active state | Querying a name that has lapsed | Renew the name during the grace period, or wait until it becomes claimable |
| `Unauthorized` | 5 | The caller is not the owner or an authorized delegate | Using the wrong signer | Switch to the owner or admin signer profile |
| `MetadataTooLong` | 6 | The metadata URI string exceeds the maximum length | Passing a very long metadata URL | Shorten the URI (use IPFS CIDs rather than long HTTP URLs) |
| `Validation` | 7 | Generic input validation failure | Malformed name, empty owner, or invalid label | Check the name format: lowercase, `a-z0-9-`, min 3 chars, ends with `.xlm` |
| `InvalidExpiry` | 8 | `expires_at` is not in the future | Setting an expiry in the past | Use a timestamp that is in the future |
| `InvalidGracePeriod` | 9 | `grace_period_ends_at` is before `expires_at` | Misconfiguring period times | Ensure `grace_period_ends_at >= expires_at` |
| `UpgradeFailed` | 10 | The contract upgrade was rejected | Wrong wasm hash or unauthorized signer | Verify the admin keypair and wasm hash, then retry |
| `Locked` | 11 | The name is frozen for dispute resolution | A dispute lock was placed by an admin | Wait for the lock to expire or contact the admin |

---

## Registrar Contract

**Source:** `contracts/registrar/src/lib.rs`

The registrar accepts public registrations and renewals.  It validates labels,
enforces pricing, and calls into the registry.

| Error Name | Code | Description | Common Cause | Resolution |
|------------|------|-------------|--------------|------------|
| `InsufficientFee` | 1 | The fee paid is less than the required registration fee | Underfunded account or stale fee estimate | Fund the account; run `xlm-ns quote <name> <years>` to get the current fee |
| `NotFound` | 2 | The name does not exist in the registrar | Trying to renew an unregistered name | Register the name first with `xlm-ns register` |
| `NotRenewable` | 3 | The name cannot be renewed in its current state | Attempting renewal when name is expired past grace | Wait until the name is claimable and re-register it |
| `AlreadyRegistered` | 4 | The name is already registered by another account | Race condition or duplicate registration attempt | Use `xlm-ns whois <name>` to find the current owner |
| `Reserved` | 5 | The label is on the reserved list and cannot be publicly registered | Attempting to register a protected label | Choose a different label; contact admin for reserved-name requests |
| `Unauthorized` | 6 | The signer is not the owner or an authorized delegate | Using the wrong keypair | Switch to the name's owner signer profile |
| `Validation` | 7 | The label or input failed validation | Invalid characters, wrong length, or missing TLD | Use a label matching `[a-z0-9][a-z0-9-]{1,}[a-z0-9].xlm` |
| `RegistrationClaimable` | 8 | The name is past the grace period and claimable, not renewable | Trying to renew an expired-and-claimable name | Use `xlm-ns register` to claim the name as new |
| `NotInitialized` | 9 | The registrar contract has not been initialized | Running against a fresh deployment | Deploy and initialize the registrar contract before use |
| `AlreadyInitialized` | 10 | `initialize` was called on an already-initialized contract | Duplicate initialization | Skip initialization; the contract is already set up |
| `RateLimitExceeded` | 11 | Too many registration attempts in the rate-limit window | Batch-registering many names too quickly | Wait for the rate-limit window to reset; spread registrations over time |
| `UpgradeFailed` | 12 | The contract upgrade was rejected | Wrong wasm hash or unauthorized signer | Verify the admin keypair and wasm hash, then retry |
| `QuoteExpired` | 13 | The registration quote has expired before use | Waiting too long between `quote` and `register` | Run `xlm-ns quote <name> <years>` to get a fresh quote and use it immediately |

---

## Resolver Contract

**Source:** `contracts/resolver/src/lib.rs`

The resolver stores off-chain data records (address mappings, text records,
content hashes, cross-chain addresses) for a registered name.

| Error Name | Code | Description | Common Cause | Resolution |
|------------|------|-------------|--------------|------------|
| `Validation` | 1 | The input failed validation | Malformed chain name, empty key, or invalid address | Check that all input fields are non-empty and correctly formatted |
| `RecordNotFound` | 2 | No resolver record exists for the requested name | Resolving a name that has no records set | Set a resolver record first with `xlm-ns record set` |
| `Unauthorized` | 3 | The caller is not the name owner or authorized delegate | Using the wrong keypair | Switch to the owner signer profile |
| `TooManyTextRecords` | 4 | The name already has the maximum allowed text records | Adding records to a name that is at capacity | Remove unused records with `xlm-ns record delete` before adding new ones |
| `NotInitialized` | 5 | The resolver contract has not been initialized | Running against a fresh deployment | Deploy and initialize the resolver contract before use |
| `TextRecordValueTooLong` | 6 | The text record value exceeds the maximum length | Setting a very long string value | Shorten the value; store large data off-chain and link via IPFS |
| `InvalidChain` | 7 | The chain identifier is not recognized by the resolver | Passing an unsupported chain name for cross-chain records | Use a supported chain name (e.g. `eth`, `btc`, `sol`) |
| `InvalidKey` | 8 | The text record key is malformed or not normalized | Using uppercase or special characters in the key | Normalize the key to lowercase ASCII |
| `BatchTooLarge` | 9 | The batch update contains too many operations | Sending a large batch of record updates | Split the request into smaller batches |
| `UpgradeFailed` | 10 | The contract upgrade was rejected | Wrong wasm hash or unauthorized signer | Verify the admin keypair and wasm hash, then retry |

---

## Subdomain Contract

**Source:** `contracts/subdomain/src/lib.rs`

The subdomain contract manages hierarchical name delegation, allowing a parent
domain owner to create and manage subdomain records.

| Error Name | Code | Description | Common Cause | Resolution |
|------------|------|-------------|--------------|------------|
| `Validation` | 1 | The subdomain label or input failed validation | Invalid label characters or format | Use a label matching `[a-z0-9][a-z0-9-]*[a-z0-9]` |
| `ParentNotFound` | 2 | The parent domain is not registered | Trying to create a subdomain under an unregistered name | Register the parent domain first, then create the subdomain |
| `AlreadyExists` | 3 | A subdomain with that label already exists | Duplicate creation attempt | Choose a different subdomain label, or delete the existing one first |
| `NotFound` | 4 | The subdomain does not exist | Querying a subdomain that was never created | Check the subdomain path spelling |
| `Unauthorized` | 5 | The caller is not the parent owner or authorized controller | Using the wrong keypair | Use the parent domain's owner signer |
| `UpgradeFailed` | 6 | The contract upgrade was rejected | Wrong wasm hash or unauthorized signer | Verify the admin keypair and wasm hash, then retry |
| `DepthLimitExceeded` | 7 | The subdomain path is nested too deeply | Creating a subdomain beyond the depth limit | Use a shallower subdomain hierarchy |

---

## Auction Contract

**Source:** `contracts/auction/src/lib.rs`

The auction contract manages English-style auctions for disputed or reserved
names.  It enforces bid ordering, time windows, and reentrancy protection.

| Error Name | Code | Description | Common Cause | Resolution |
|------------|------|-------------|--------------|------------|
| `Validation` | 1 | The auction parameters failed validation | Invalid timestamps, zero reserve price, or empty name | Check start time < end time and reserve price > 0 |
| `AlreadyExists` | 2 | An auction for this name is already running | Attempting to create a duplicate auction | Use `xlm-ns auction status <name>` to inspect the current auction |
| `NotFound` | 3 | No auction exists for this name | Bidding on or settling a name with no auction | Create an auction with `xlm-ns auction create` first |
| `AuctionClosed` | 4 | The auction is closed and no longer accepting bids | Bidding after the end time | The auction has ended; use settle or start a new one |
| `AuctionNotStarted` | 5 | The auction start time has not been reached | Bidding before the auction begins | Wait until the auction start timestamp and try again |
| `AuctionNotEnded` | 6 | The auction has not ended yet; settlement is not available | Attempting to settle before the end time | Wait until the auction end timestamp, then settle |
| `AlreadySettled` | 7 | The auction was already finalized | Settling an already-completed auction | Check the auction result with `xlm-ns auction result <name>` |
| `InvalidBid` | 8 | The bid is below the reserve price or minimum increment | Underbidding | Increase the bid; check the current highest bid with `xlm-ns auction status <name>` |
| `UpgradeFailed` | 9 | The contract upgrade was rejected | Wrong wasm hash or unauthorized signer | Verify the admin keypair and wasm hash, then retry |
| `ReentrancyDetected` | 10 | A reentrancy guard blocked a concurrent call | A previous call is still in flight | Wait for the in-flight transaction to complete before retrying |

---

## Bridge Contract

**Source:** `contracts/bridge/src/lib.rs`

The bridge contract manages cross-chain resolver routing, mapping external
blockchain names to their on-chain resolver contracts.

| Error Name | Code | Description | Common Cause | Resolution |
|------------|------|-------------|--------------|------------|
| `Validation` | 1 | The bridge input failed validation | Empty chain name or invalid resolver address | Verify the chain name and contract address format |
| `UnsupportedChain` | 2 | The target chain is not registered in the bridge | Using a chain name that was never added | Use `xlm-ns bridge list` to see supported chains; add new chains via admin |
| `UpgradeFailed` | 3 | The contract upgrade was rejected | Wrong wasm hash or unauthorized signer | Verify the admin keypair and wasm hash, then retry |
| `Unauthorized` | 4 | The caller is not the bridge admin | Using a non-admin signer for admin operations | Use the bridge admin signer profile |
| `NotFound` | 5 | The chain or route was not found | Querying a chain that was never registered | Check the chain name with `xlm-ns bridge list` |
| `AlreadyExists` | 6 | The chain or route is already registered | Attempting to add a duplicate chain entry | Update the existing route with `xlm-ns bridge update` instead |

---

## NFT Contract

**Source:** `contracts/nft/src/lib.rs`

The NFT contract represents ownership of `.xlm` names as transferable NFTs.
Each name is a unique token; token IDs are derived from the name string.

| Error Name | Code | Description | Common Cause | Resolution |
|------------|------|-------------|--------------|------------|
| `AlreadyMinted` | 1 | A token for this name has already been minted | Attempting to mint a duplicate name token | The name is already owned; use transfer if you want to take ownership |
| `NotFound` | 2 | The token does not exist | Querying a token for a name that was never registered | Register the name first; the NFT is minted automatically |
| `Unauthorized` | 3 | The caller is not the token owner or admin | Using the wrong keypair for a transfer or burn | Use the token owner signer or the NFT admin signer |
| `UpgradeFailed` | 4 | The contract upgrade was rejected | Wrong wasm hash or unauthorized signer | Verify the admin keypair and wasm hash, then retry |
| `NotInitialized` | 5 | The NFT contract has not been initialized | Running against a fresh deployment | Deploy and initialize the NFT contract before use |

---

## Troubleshooting Scenarios

### Registration Failure — Insufficient Fee

```
Error: register failed: HostError: Error(Contract, #1)
```

The error is in the **registrar** contract (code 1 = `InsufficientFee`).

**Steps:**
1. Run `xlm-ns quote <name> <years>` to get the current fee.
2. Fund your account with at least that amount plus network fees.
3. Re-run the registration.

---

### Name Already Taken

```
Error: register failed: HostError: Error(Contract, #4)
```

The error is in the **registrar** contract (code 4 = `AlreadyRegistered`).

**Steps:**
1. Run `xlm-ns whois <name>` to see the current owner and expiry.
2. If the name is in the grace period, wait for it to become claimable.
3. Choose a different name.

---

### Quote Expired Before Registration

```
Error: register failed: HostError: Error(Contract, #13)
```

The error is in the **registrar** contract (code 13 = `QuoteExpired`).

**Steps:**
1. Run `xlm-ns quote <name> <years>` to generate a fresh quote.
2. Complete the registration immediately — quotes have a short TTL.

---

### Resolver Record Not Found

```
Error: resolve failed: HostError: Error(Contract, #2)
```

The error is in the **resolver** contract (code 2 = `RecordNotFound`).

**Steps:**
1. Check that the name is registered: `xlm-ns whois <name>`.
2. Set a resolver record: `xlm-ns record set <name> --address <addr>`.
3. Retry the resolve operation.

---

### Subdomain Parent Missing

```
Error: subdomain create failed: HostError: Error(Contract, #2)
```

The error is in the **subdomain** contract (code 2 = `ParentNotFound`).

**Steps:**
1. Register the parent domain first: `xlm-ns register <parent.xlm>`.
2. Then create the subdomain: `xlm-ns subdomain create <sub.parent.xlm>`.

---

### Auction Bid Too Low

```
Error: bid failed: HostError: Error(Contract, #8)
```

The error is in the **auction** contract (code 8 = `InvalidBid`).

**Steps:**
1. Query the current bid: `xlm-ns auction status <name>`.
2. Submit a bid at least 5 % above the current highest bid.

---

## SDK Error Handling Examples

### Contract-Aware Decoding

```rust
use xlm_ns_sdk::errors::{decode_error, ContractErrorCode, SdkError};

fn handle_sdk_error(err: &SdkError) {
    match err {
        SdkError::ContractError(code) => {
            // ContractErrorCode implements Display with descriptive messages
            eprintln!("Contract error: {}", code);
        }
        SdkError::ContractInvocationFailed { reason, .. } => {
            // Parse the raw error string when the contract name is known
            if let Some(num) = extract_code(reason) {
                let typed = decode_error("registrar", num);
                eprintln!("Registrar error: {}", typed);
            }
        }
        _ => eprintln!("SDK error: {}", err),
    }
}

fn extract_code(reason: &str) -> Option<u32> {
    // Matches "Error(Contract, #N)"
    let re = regex::Regex::new(r"Error\(Contract, #(\d+)\)").ok()?;
    re.captures(reason)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}
```

### Matching Specific Errors

```rust
use xlm_ns_sdk::errors::{ContractErrorCode, SdkError};

fn register_with_handling(client: &Client, name: &str) -> Result<(), SdkError> {
    match client.register(name) {
        Err(SdkError::ContractError(ContractErrorCode::RegistrarInsufficientFee)) => {
            eprintln!("Fee too low. Run `xlm-ns quote {}` for the current price.", name);
            Err(SdkError::ContractError(ContractErrorCode::RegistrarInsufficientFee))
        }
        Err(SdkError::ContractError(ContractErrorCode::RegistrarAlreadyRegistered)) => {
            eprintln!("'{}' is already taken.", name);
            Err(SdkError::ContractError(ContractErrorCode::RegistrarAlreadyRegistered))
        }
        Err(SdkError::ContractError(ContractErrorCode::RegistrarQuoteExpired)) => {
            eprintln!("Quote expired. Fetch a fresh quote and retry.");
            // refresh quote logic here, then retry
            client.register(name)
        }
        other => other,
    }
}
```

---

## CLI Error Handling Examples

### Registration

```bash
$ xlm-ns register alice.xlm --years 1
Error: alice.xlm requires a higher registration fee.
Suggestion: Fund the account, then rerun `xlm-ns quote alice.xlm 1` to verify the cost.
Docs: docs/error-reference.md
```

### Resolve

```bash
$ xlm-ns resolve bob.xlm
Error: bob.xlm has no resolver record.
Suggestion: Use `xlm-ns whois bob.xlm` or register a resolver record first.
Docs: docs/error-reference.md
```

### Verbose mode

Pass `--verbose` to any command to see the raw technical error chain:

```bash
$ xlm-ns register alice.xlm --years 1 --verbose
Error: alice.xlm requires a higher registration fee. Suggestion: ...
Technical details: contract error: RegistrarInsufficientFee | status 400.
```

---

## Related Issues and Future Work

| Issue | Topic |
|-------|-------|
| [#492](https://github.com/Soroban-Ens/xlm-ens/issues/492) | SDK typed error hierarchy — builds on top of this reference |
| [#476](https://github.com/Soroban-Ens/xlm-ens/issues/476) | CLI improved error messages — references error codes documented here |
| [#438](https://github.com/Soroban-Ens/xlm-ens/issues/438) | SDK comprehensive API documentation — incorporates this reference |
