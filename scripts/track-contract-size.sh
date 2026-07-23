#!/usr/bin/env bash
# Record contract WASM sizes, compare them with an optional main-branch
# baseline, and enforce the early-warning budget threshold.
set -euo pipefail

CONFIG_FILE="${1:-.github/contract-size-budget.json}"
WASM_DIR="${2:-target/wasm32v1-none/release}"
BASELINE_FILE="${3:-}"
SIZES_FILE="${4:-artifacts/contract-sizes.json}"
REPORT_FILE="${5:-artifacts/contract-size-trend.md}"
GROWTH_WARNING_PERCENT="${GROWTH_WARNING_PERCENT:-5}"
BUDGET_FAILURE_PERCENT="${BUDGET_FAILURE_PERCENT:-90}"

if ! command -v jq >/dev/null 2>&1; then
    echo "Error: jq is required but not installed." >&2
    exit 1
fi

mkdir -p "$(dirname "$SIZES_FILE")" "$(dirname "$REPORT_FILE")"

baseline='{"contracts":[]}'
if [[ -n "$BASELINE_FILE" && -f "$BASELINE_FILE" ]]; then
    baseline=$(cat "$BASELINE_FILE")
    echo "Comparing against baseline: $BASELINE_FILE"
else
    echo "::notice::No main-branch contract-size baseline is available; recording an initial baseline."
fi

format_kib() {
    awk -v bytes="$1" 'BEGIN { printf "%.1f KiB", bytes / 1024 }'
}

format_percent() {
    awk -v numerator="$1" -v denominator="$2" 'BEGIN { printf "%.1f%%", numerator * 100 / denominator }'
}

contracts='[]'
markdown_rows=''
growth_warning=0
budget_failure=0

while IFS=$'\t' read -r name max_size; do
    wasm_file="$WASM_DIR/$name.wasm"
    if [[ ! -f "$wasm_file" ]]; then
        echo "Error: expected WASM output not found: $wasm_file" >&2
        exit 1
    fi

    size=$(wc -c < "$wasm_file" | tr -d '[:space:]')
    utilization=$(format_percent "$size" "$max_size")
    contracts=$(jq \
        --arg name "$name" \
        --argjson size "$size" \
        --argjson max_size "$max_size" \
        '. + [{name: $name, size_bytes: $size, max_size_bytes: $max_size,
                budget_utilization_percent: ($size * 100 / $max_size)}]' \
        <<<"$contracts")

    baseline_size=$(jq -r --arg name "$name" \
        '.contracts[]? | select(.name == $name) | .size_bytes' <<<"$baseline" | head -n 1)
    delta='— (no baseline)'
    status='✅ Stable'
    if [[ -n "$baseline_size" && "$baseline_size" != "null" ]]; then
        difference=$((size - baseline_size))
        if (( difference >= 0 )); then
            sign='+'
        else
            sign=''
        fi
        delta="$sign$(format_kib "$difference") ($sign$(format_percent "$difference" "$baseline_size"))"
        if (( difference > 0 )) && awk -v delta="$difference" -v base="$baseline_size" -v threshold="$GROWTH_WARNING_PERCENT" 'BEGIN { exit !(delta * 100 / base > threshold) }'; then
            status="⚠️ Growth > ${GROWTH_WARNING_PERCENT}%"
            growth_warning=1
            echo "::warning::$name grew by $(format_percent "$difference" "$baseline_size") relative to main."
        fi
    fi

    if (( size * 100 > max_size * BUDGET_FAILURE_PERCENT )); then
        status='❌ Above 90% budget'
        budget_failure=1
        echo "::error::$name is using $utilization of its WASM size budget."
    fi

    markdown_rows+="| $name | $(format_kib "$size") | $(format_kib "$max_size") | $utilization | $delta | $status |"$'\n'
done < <(jq -r '.[] | [.name, .max_size_bytes] | @tsv' "$CONFIG_FILE")

jq -n \
    --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg source_ref "${GITHUB_SHA:-local}" \
    --argjson contracts "$contracts" \
    '{schema_version: 1, generated_at: $generated_at, source_ref: $source_ref, contracts: $contracts}' \
    > "$SIZES_FILE"

{
    echo "<!-- contract-size-trend -->"
    echo "## Contract WASM size trend"
    echo
    echo "| Contract | Current | Budget | Utilization | Δ vs main | Status |"
    echo "|---|---:|---:|---:|---:|---|"
    printf '%s' "$markdown_rows"
    echo
    if (( growth_warning )); then
        echo "> ⚠️ At least one contract grew by more than ${GROWTH_WARNING_PERCENT}% compared with main."
    fi
    if (( budget_failure )); then
        echo "> ❌ At least one contract exceeds the ${BUDGET_FAILURE_PERCENT}% early-warning budget threshold."
    fi
} > "$REPORT_FILE"

echo "Size history written to $SIZES_FILE"
echo "Trend report written to $REPORT_FILE"
cat "$REPORT_FILE"

if (( budget_failure )); then
    exit 1
fi
