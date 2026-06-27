# Reentrancy Audit

This document maps the cross-contract call graph in the `xlm-ns` contracts and records where reentrancy was assessed.

## Summary

- The only runtime reentrancy guard added in this pass is in `auction::place_bid`, where the contract called an external token contract before recording the bid.
- The remaining mutating call chains either:
  - perform state writes before their external call and only target admin-configured contracts, or
  - perform read-only calls before their writes.
- No public API in the current workspace accepts an arbitrary caller-supplied contract address and then invokes it directly.

## Call Graph

### `contracts/registry`

- `register(...)`
  - Writes registry ownership state.
  - Calls configured NFT contract `mint(...)` if an NFT contract address is configured.
- `renew(...)`
  - Writes registry expiry state.
  - Calls configured NFT contract `sync_expiry(...)` if configured.
- `update_owner(...)`
  - Internal mutation only.
  - No external call.

### `contracts/registrar`

- `register(...)`
  - Writes registrar registration state, treasury, and counters.
  - Calls configured registry contract `register(...)`.
- `renew(...)`
  - Writes registrar renewal state, treasury, and counters.
  - Calls configured registry contract `renew(...)`.

### `contracts/resolver`

- `set_record(...)`, `set_address(...)`, `set_primary_name(...)`, `batch_set(...)`
  - Mutate resolver storage.
  - Consult the registry via read-only `resolve(...)` in authorization helpers before writes.
- `remove_record(...)`, `update_owner(...)`, `transfer_record_owner(...)`
  - Internal mutations only.
  - No external call.

### `contracts/auction`

- `place_bid(...)`
  - Calls the configured token contract `transfer(...)` before recording the bid.
  - This path is reentrancy-sensitive and now uses a storage-backed lock.
- `settle(...)`
  - Writes settlement state before any token payouts.
  - Token payout calls happen after settlement is recorded.

### `contracts/subdomain`

- No `env.invoke_contract(...)` usage.
- All mutations are local storage updates.

### `contracts/nft`

- `mint(...)`
  - Writes NFT ownership state.
  - Reads the registry via `resolve(...)` to cache metadata.
- `refresh_name_data(...)`
  - Reads the registry via `resolve(...)`.
  - Writes cached metadata after the read.
- `transfer(...)`, `transfer_from(...)`
  - Write NFT ownership state.
  - Call configured registry `update_owner(...)` to keep ownership aligned.

### `contracts/bridge`

- No `env.invoke_contract(...)` usage.
- The contract builds and stores routing metadata locally.

## Trust Boundaries

- `registry` and `nft` contract addresses are instance-configured and admin-controlled.
- `registrar` stores the registry address in instance storage and calls it as a trusted dependency.
- `bridge` stores resolver addresses in admin-controlled supported-chain records.
- `auction::place_bid` is the highest-risk path because the token address is an external dependency and the token callback occurs before the bid is committed.

## Mitigation Notes

- `auction::place_bid` now uses a `ReentrancyLock` storage flag.
- If future changes add user-configurable contract addresses to public mutating calls, those paths should either:
  - move all state writes before external calls, or
  - be wrapped in the same storage-backed guard pattern.
