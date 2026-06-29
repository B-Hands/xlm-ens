#!/usr/bin/env bash
# Deploy all xlm-ns contracts to a local sandbox and run smoke tests.
#
# Required environment variables:
#   SOROBAN_RPC_URL
#   SOROBAN_NETWORK_PASSPHRASE
#
# Expects contract WASM files to be in `artifacts/wasm/`.

set -euo pipefail

# ── Test Fixtures ────────────────────────────────────────────────────────────

# Use the same admin account for all contracts to keep things simple.
ADMIN_ACCOUNT_NAME="xlm-ns-admin"
ADMIN_ACCOUNT_SECRET="SAPS2Q2K32QT36MX4G77Z3M24Q3S2G5Z5O3F4Y2X2Z2Y2R2V2A2Z"

# ── Contract Deployment ──────────────────────────────────────────────────────

echo "---"
echo "Deploying contracts"

# All contracts must be deployed from a funded account. The stellar/quickstart
# container helpfully provides a default identity with tokens.
soroban config identity address default

# Deploy each contract and capture its address for the next step.
# The deployment order matters because contracts depend on each other.
REGISTRY_WASM="artifacts/wasm/xlm_ns_registry.wasm"
REGISTRAR_WASM="artifacts/wasm/xlm_ns_registrar.wasm"
RESOLVER_WASM="artifacts/wasm/xlm_ns_resolver.wasm"
AUCTION_WASM="artifacts/wasm/xlm_ns_auction.wasm"
SUBDOMAIN_WASM="artifacts/wasm/xlm_ns_subdomain.wasm"
NFT_WASM="artifacts/wasm/xlm_ns_nft.wasm"
BRIDGE_WASM="artifacts/wasm/xlm_ns_bridge.wasm"

REGISTRY_ID=$(soroban contract deploy --wasm "$REGISTRY_WASM" --source default)
echo "✓ Registry: $REGISTRY_ID"

# The registrar needs the registry address at initialization.
REGISTRAR_ID=$(soroban contract deploy --wasm "$REGISTRAR_WASM" --source default)
soroban contract invoke --id "$REGISTRAR_ID" --source default -- \
  initialize --registry "$REGISTRY_ID" --admin "$(soroban config identity address default)"
echo "✓ Registrar: $REGISTRAR_ID"

# The resolver also needs the registry address.
RESOLVER_ID=$(soroban contract deploy --wasm "$RESOLVER_WASM" --source default)
soroban contract invoke --id "$RESOLVER_ID" --source default -- \
  initialize --registry_id "$REGISTRY_ID"
echo "✓ Resolver: $RESOLVER_ID"

# The auction contract needs the registrar and registry addresses.
AUCTION_ID=$(soroban contract deploy --wasm "$AUCTION_WASM" --source default)
soroban contract invoke --id "$AUCTION_ID" --source default -- \
  initialize --admin "$(soroban config identity address default)"
echo "✓ Auction: $AUCTION_ID"

# The subdomain contract needs the registrar address.
SUBDOMAIN_ID=$(soroban contract deploy --wasm "$SUBDOMAIN_WASM" --source default)
soroban contract invoke --id "$SUBDOMAIN_ID" --source default -- \
  initialize --admin "$(soroban config identity address default)"
echo "✓ Subdomain: $SUBDOMAIN_ID"

# The NFT contract needs the registry address.
NFT_ID=$(soroban contract deploy --wasm "$NFT_WASM" --source default)
soroban contract invoke --id "$NFT_ID" --source default -- \
  initialize --admin "$(soroban config identity address default)"
echo "✓ NFT: $NFT_ID"

# The bridge contract needs the resolver address.
BRIDGE_ID=$(soroban contract deploy --wasm "$BRIDGE_WASM" --source default) 
soroban contract invoke --id "$BRIDGE_ID" --source default -- \
  initialize --admin "$(soroban config identity address default)"
echo "✓ Bridge: $BRIDGE_ID"

echo "✓ All contracts deployed successfully."

# ── Smoke Tests ──────────────────────────────────────────────────────────────

echo "---"
echo "Running smoke tests"

# 1. Register a name
echo "Registering 'test.xlm'..."
soroban contract invoke --id "$REGISTRAR_ID" --source default -- 
  register --name "test" --tld "xlm" --owner "$(soroban config identity address default)" --duration_in_years 1

# 2. Resolve the name
echo "Resolving 'test.xlm'..."
RESOLUTION=$(soroban contract invoke --id "$RESOLVER_ID" --source default -- resolve --name "test.xlm")
# Check if the resolution result contains the owner's address
if [[ "$RESOLUTION" != *"$(soroban config identity address default)"* ]]; then
  echo "ERROR: could not resolve 'test.xlm' to the correct owner" >&2
  exit 1
fi
echo "✓ Resolution successful."

# 3. Transfer the name
echo "Transferring 'test.xlm'..."
soroban contract invoke --id "$REGISTRY_ID" --source default -- 
  transfer --name "test.xlm" --new_owner "$(soroban config identity address default)" # Placeholder, should be a different address
echo "✓ Transfer successful (placeholder)."

# TODO: Capture and report budget metrics for each operation.

echo "---"
echo "✓ Smoke tests passed."
