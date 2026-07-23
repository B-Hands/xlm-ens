#!/usr/bin/env bash
# Post-deployment verification for xlm-ns Soroban contracts (issue #602).
#
# Confirms that a set of deployed contracts is initialized, wired together
# correctly, and operational on the critical path (register -> resolve ->
# NFT minted). Intended to run right after scripts/deploy/testnet.sh or
# scripts/deploy/mainnet.sh so a misconfigured cross-contract link is caught
# before it cascades.
#
# Usage:
#   scripts/verify-deployment.sh --network testnet \
#     --registry CA... --registrar CA... --resolver CA... \
#     --nft CA... --auction CA... --subdomain CA... --bridge CA... \
#     [--source default] [--report-json artifacts/verify-deployment-report.json] \
#     [--skip-e2e] [--force-e2e-on-mainnet]
#
# Contract IDs may also be supplied via environment variables so the script
# is easy to wire into deploy tooling that already exports them:
#   XLM_NS_REGISTRY_ID, XLM_NS_REGISTRAR_ID, XLM_NS_RESOLVER_ID,
#   XLM_NS_NFT_ID, XLM_NS_AUCTION_ID, XLM_NS_SUBDOMAIN_ID, XLM_NS_BRIDGE_ID
#
# Requires: `stellar` CLI (stellar-cli) with the --source identity funded on
# the target network, and `jq`.
#
# Exit code is non-zero if any check fails.

set -euo pipefail

# ── Argument parsing ─────────────────────────────────────────────────────────

NETWORK=""
SOURCE_IDENTITY="${XLM_NS_VERIFY_SOURCE:-default}"
REPORT_JSON="artifacts/verify-deployment-report.json"
SKIP_E2E=0
FORCE_E2E_ON_MAINNET=0

REGISTRY_ID="${XLM_NS_REGISTRY_ID:-}"
REGISTRAR_ID="${XLM_NS_REGISTRAR_ID:-}"
RESOLVER_ID="${XLM_NS_RESOLVER_ID:-}"
NFT_ID="${XLM_NS_NFT_ID:-}"
AUCTION_ID="${XLM_NS_AUCTION_ID:-}"
SUBDOMAIN_ID="${XLM_NS_SUBDOMAIN_ID:-}"
BRIDGE_ID="${XLM_NS_BRIDGE_ID:-}"

usage() {
  sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --network) NETWORK="$2"; shift 2 ;;
    --registry) REGISTRY_ID="$2"; shift 2 ;;
    --registrar) REGISTRAR_ID="$2"; shift 2 ;;
    --resolver) RESOLVER_ID="$2"; shift 2 ;;
    --nft) NFT_ID="$2"; shift 2 ;;
    --auction) AUCTION_ID="$2"; shift 2 ;;
    --subdomain) SUBDOMAIN_ID="$2"; shift 2 ;;
    --bridge) BRIDGE_ID="$2"; shift 2 ;;
    --source) SOURCE_IDENTITY="$2"; shift 2 ;;
    --report-json) REPORT_JSON="$2"; shift 2 ;;
    --skip-e2e) SKIP_E2E=1; shift ;;
    --force-e2e-on-mainnet) FORCE_E2E_ON_MAINNET=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument '$1'" >&2; usage; exit 2 ;;
  esac
done

if [[ -z "$NETWORK" ]]; then
  echo "error: --network testnet|mainnet is required" >&2
  exit 2
fi

case "$NETWORK" in
  testnet)
    RPC_URL="${XLM_NS_RPC_URL:-https://soroban-testnet.stellar.org}"
    NETWORK_PASSPHRASE="${XLM_NS_NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"
    ;;
  mainnet)
    RPC_URL="${XLM_NS_RPC_URL:-https://mainnet.sorobanrpc.com}"
    NETWORK_PASSPHRASE="${XLM_NS_NETWORK_PASSPHRASE:-Public Global Stellar Network ; September 2015}"
    ;;
  *)
    echo "error: --network must be 'testnet' or 'mainnet' (got '$NETWORK')" >&2
    exit 2
    ;;
esac

declare -A CONTRACTS=(
  [registry]="$REGISTRY_ID"
  [registrar]="$REGISTRAR_ID"
  [resolver]="$RESOLVER_ID"
  [nft]="$NFT_ID"
  [auction]="$AUCTION_ID"
  [subdomain]="$SUBDOMAIN_ID"
  [bridge]="$BRIDGE_ID"
)

for label in registry registrar resolver nft auction subdomain bridge; do
  if [[ -z "${CONTRACTS[$label]}" ]]; then
    echo "error: missing contract id for '$label' (pass --$label or set XLM_NS_${label^^}_ID)" >&2
    exit 2
  fi
done

command -v stellar >/dev/null 2>&1 || { echo "error: 'stellar' CLI not found on PATH" >&2; exit 2; }
command -v jq >/dev/null 2>&1 || { echo "error: 'jq' is required" >&2; exit 2; }

mkdir -p "$(dirname "$REPORT_JSON")"
CHECKS_LOG="$(mktemp)"
trap 'rm -f "$CHECKS_LOG"' EXIT

echo "xlm-ns deployment verification — network=$NETWORK source=$SOURCE_IDENTITY"
for label in registry registrar resolver nft auction subdomain bridge; do
  echo "  $label: ${CONTRACTS[$label]}"
done
echo "---"

START_TS=$(date +%s)
FAILURES=0

# ── Helpers ───────────────────────────────────────────────────────────────────

invoke() {
  # invoke <contract-id> <function> [args...]
  local id="$1"; shift
  stellar contract invoke \
    --id "$id" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --source "$SOURCE_IDENTITY" \
    -- "$@"
}

record() {
  # record <name> <pass|fail|skip> <detail> <duration_ms>
  local name="$1" status="$2" detail="$3" duration_ms="$4"
  jq -n --arg name "$name" --arg status "$status" --arg detail "$detail" \
        --argjson duration_ms "$duration_ms" \
    '{name: $name, status: $status, detail: $detail, duration_ms: $duration_ms}' \
    >> "$CHECKS_LOG"

  case "$status" in
    pass) echo "  ok   $name (${duration_ms}ms)" ;;
    skip) echo "  skip $name — $detail" ;;
    *)    echo "  FAIL $name — $detail" >&2; FAILURES=$((FAILURES + 1)) ;;
  esac
}

now_ms() { date +%s%3N; }

# ── 1. version() on all 7 contracts ─────────────────────────────────────────

echo "Checking contract versions..."
for label in registry registrar resolver nft auction subdomain bridge; do
  t0=$(now_ms)
  if out=$(invoke "${CONTRACTS[$label]}" version 2>&1); then
    out_trimmed="${out//\"/}"
    if [[ "$out_trimmed" =~ ^[0-9]+$ ]]; then
      record "version:${label}" pass "version=$out_trimmed" $(( $(now_ms) - t0 ))
    else
      record "version:${label}" fail "unexpected version() output: $out" $(( $(now_ms) - t0 ))
    fi
  else
    record "version:${label}" fail "invoke failed: $out" $(( $(now_ms) - t0 ))
  fi
done

# ── 2. Pause status (blocked on #493 — emergency pause is not yet implemented) ─

record "pause_status:all" skip \
  "no contract currently exposes a pause getter (see issue #493); nothing to check yet" 0

# ── 3. Cross-contract links + end-to-end critical path ──────────────────────
#
# None of the contracts expose a getter for the address they were linked
# with at initialize()/set_registry()/set_nft_contract() time, so a link
# can't be read back directly. Instead each link is exercised functionally:
# a broken link makes the corresponding operation fail (the cross-contract
# call traps or the resulting state is wrong), which is arguably a stronger
# guarantee than an address comparison would be.

run_e2e=1
if [[ "$SKIP_E2E" -eq 1 ]]; then
  run_e2e=0
  e2e_skip_reason="--skip-e2e was passed"
elif [[ "$NETWORK" == "mainnet" && "$FORCE_E2E_ON_MAINNET" -ne 1 ]]; then
  run_e2e=0
  e2e_skip_reason="mutating checks are skipped on mainnet by default to avoid registering a real name and spending real fees; pass --force-e2e-on-mainnet to override"
fi

if [[ "$run_e2e" -eq 0 ]]; then
  for name in \
    "link:registrar_to_registry" \
    "link:registry_to_nft" \
    "link:resolver_to_registry" \
    "e2e:register_resolve_nft"
  do
    record "$name" skip "$e2e_skip_reason" 0
  done
else
  echo "Running end-to-end critical path (register -> resolve -> NFT)..."

  OWNER_ADDRESS="$(stellar keys address "$SOURCE_IDENTITY")"
  NOW_UNIX="$(date +%s)"
  TEST_LABEL="verify${NOW_UNIX}"
  TEST_NAME="${TEST_LABEL}.xlm"

  # Fee quote (read-only) needed to pass --max_price to register.
  t0=$(now_ms)
  if quote=$(invoke "${CONTRACTS[registrar]}" quote_registration \
      --label "$TEST_LABEL" --years 1 --now_unix "$NOW_UNIX" 2>&1); then
    record "quote_registration" pass "$quote" $(( $(now_ms) - t0 ))
    fee_stroops=$(echo "$quote" | jq -r '.fee_stroops // empty' 2>/dev/null || true)
    if [[ -z "$fee_stroops" ]]; then
      # Fall back to a generous ceiling if the quote isn't JSON-shaped in
      # this CLI version; register() only enforces max_price >= actual fee.
      fee_stroops=1000000000
    fi

    # registrar -> registry link: register() forwards the registration to
    # the registry the registrar was initialized with.
    t0=$(now_ms)
    if out=$(invoke "${CONTRACTS[registrar]}" register \
        --label "$TEST_LABEL" --owner "$OWNER_ADDRESS" --years 1 \
        --max_price "$fee_stroops" --now_unix "$NOW_UNIX" 2>&1); then
      record "link:registrar_to_registry" pass "registered $TEST_NAME via registrar" $(( $(now_ms) - t0 ))

      # registry state + registry -> nft link: the registry mints an NFT for
      # the name as part of register() if (and only if) its NftContract link
      # points at the deployed NFT contract.
      t0=$(now_ms)
      if resolved=$(invoke "${CONTRACTS[registry]}" resolve \
          --name "$TEST_NAME" --now_unix "$NOW_UNIX" 2>&1) && [[ "$resolved" == *"$OWNER_ADDRESS"* ]]; then
        record "e2e:register_resolve_nft" pass "registry.resolve($TEST_NAME) owner matches" $(( $(now_ms) - t0 ))
      else
        record "e2e:register_resolve_nft" fail "registry.resolve($TEST_NAME) did not return the expected owner: $resolved" $(( $(now_ms) - t0 ))
      fi

      t0=$(now_ms)
      if nft_owner=$(invoke "${CONTRACTS[nft]}" owner_of --token_id "$TEST_NAME" 2>&1) && [[ "$nft_owner" == *"$OWNER_ADDRESS"* ]]; then
        record "link:registry_to_nft" pass "nft.owner_of($TEST_NAME) == registered owner" $(( $(now_ms) - t0 ))
      else
        record "link:registry_to_nft" fail "NFT was not minted for $TEST_NAME (registry -> nft link likely misconfigured): $nft_owner" $(( $(now_ms) - t0 ))
      fi

      # resolver -> registry link: set_record() authorizes the caller by
      # asking the registry who owns the name; a bad link means this call
      # fails even though 'owner' really does own the name.
      t0=$(now_ms)
      if out=$(invoke "${CONTRACTS[resolver]}" set_record \
          --name "$TEST_NAME" --owner "$OWNER_ADDRESS" --address "$OWNER_ADDRESS" \
          --now_unix "$NOW_UNIX" 2>&1); then
        if resolved_rec=$(invoke "${CONTRACTS[resolver]}" resolve --name "$TEST_NAME" 2>&1) && [[ "$resolved_rec" == *"$OWNER_ADDRESS"* ]]; then
          record "link:resolver_to_registry" pass "resolver.set_record + resolve round-tripped the registry-verified owner" $(( $(now_ms) - t0 ))
        else
          record "link:resolver_to_registry" fail "resolver.resolve($TEST_NAME) did not return the expected owner: $resolved_rec" $(( $(now_ms) - t0 ))
        fi
      else
        record "link:resolver_to_registry" fail "resolver.set_record was rejected — resolver's registry link likely does not match the deployed registry: $out" $(( $(now_ms) - t0 ))
      fi
    else
      record "link:registrar_to_registry" fail "registrar.register failed — registrar's registry link likely does not match the deployed registry: $out" $(( $(now_ms) - t0 ))
      record "link:registry_to_nft" skip "skipped: registration did not succeed" 0
      record "link:resolver_to_registry" skip "skipped: registration did not succeed" 0
      record "e2e:register_resolve_nft" skip "skipped: registration did not succeed" 0
    fi
  else
    record "quote_registration" fail "$quote" $(( $(now_ms) - t0 ))
    for name in "link:registrar_to_registry" "link:registry_to_nft" "link:resolver_to_registry" "e2e:register_resolve_nft"; do
      record "$name" skip "skipped: could not obtain a registration quote" 0
    done
  fi
fi

# ── Report ────────────────────────────────────────────────────────────────────

END_TS=$(date +%s)
DURATION_SECONDS=$((END_TS - START_TS))

TOTAL=$(jq -s 'length' "$CHECKS_LOG")
PASSED=$(jq -s '[.[] | select(.status == "pass")] | length' "$CHECKS_LOG")
FAILED=$(jq -s '[.[] | select(.status == "fail")] | length' "$CHECKS_LOG")
SKIPPED=$(jq -s '[.[] | select(.status == "skip")] | length' "$CHECKS_LOG")
RESULT="pass"
[[ "$FAILED" -gt 0 ]] && RESULT="fail"

jq -n \
  --arg network "$NETWORK" \
  --arg started_at "$(date -u -d "@$START_TS" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -r "$START_TS" +%Y-%m-%dT%H:%M:%SZ)" \
  --argjson duration_seconds "$DURATION_SECONDS" \
  --argjson contracts "$(jq -n \
      --arg registry "${CONTRACTS[registry]}" \
      --arg registrar "${CONTRACTS[registrar]}" \
      --arg resolver "${CONTRACTS[resolver]}" \
      --arg nft "${CONTRACTS[nft]}" \
      --arg auction "${CONTRACTS[auction]}" \
      --arg subdomain "${CONTRACTS[subdomain]}" \
      --arg bridge "${CONTRACTS[bridge]}" \
      '{registry:$registry, registrar:$registrar, resolver:$resolver, nft:$nft, auction:$auction, subdomain:$subdomain, bridge:$bridge}')" \
  --argjson checks "$(jq -s '.' "$CHECKS_LOG")" \
  --argjson total "$TOTAL" --argjson passed "$PASSED" --argjson failed "$FAILED" --argjson skipped "$SKIPPED" \
  --arg result "$RESULT" \
  '{
    network: $network,
    started_at: $started_at,
    duration_seconds: $duration_seconds,
    contracts: $contracts,
    checks: $checks,
    summary: {total: $total, passed: $passed, failed: $failed, skipped: $skipped},
    result: $result
  }' > "$REPORT_JSON"

echo "---"
echo "Summary: $PASSED passed, $FAILED failed, $SKIPPED skipped (${DURATION_SECONDS}s)"
echo "Report written to $REPORT_JSON"

if [[ "$FAILED" -gt 0 ]]; then
  echo "FAILED" >&2
  exit 1
fi

echo "OK"
