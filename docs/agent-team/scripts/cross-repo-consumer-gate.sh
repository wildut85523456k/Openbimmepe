#!/usr/bin/env bash
# cross-repo-consumer-gate.sh — Consumer gate for openbim-core candidate SHAs
#
# Usage: cross-repo-consumer-gate.sh <core-sha> [core-checkout-path]
#
# Clones/core-checks out openbim-core at the given SHA, then builds and tests
# both consumer apps (revit-alike, edesign-alike) against it via an uncommitted
# [patch] override. Restores Cargo.toml after each app.
#
# Exit codes:
#   0 = all consumers passed
#   1 = one or more consumers failed
set -euo pipefail

CORE_REPO="https://github.com/wildut85523456k/openbim-core"
CORE_URL="https://github.com/wildut85523456k/openbim-core"

# Normalize MSYS-style paths (/c/Users/...) to Windows-style (C:/Users/...)
# so native cargo can read them correctly.
normalize_path() {
    local p="$1"
    p=$(echo "$p" | sed 's|^/c/|C:/|; s|^/d/|D:/|; s|^/e/|E:/|')
    p="${p%/}"
    echo "$p"
}

if [ $# -lt 1 ]; then
    echo "Usage: $0 <core-sha> [core-checkout-path]"
    exit 2
fi

CORE_SHA="$1"
CORE_CHECKOUT="${2:-}"

# --- 1. Prepare core checkout at the candidate SHA ---
if [ -n "$CORE_CHECKOUT" ]; then
    echo "Using existing core checkout: $CORE_CHECKOUT"
else
    CORE_CHECKOUT="$HOME/.cache/openbim-core-gate-$CORE_SHA"
    rm -rf "$CORE_CHECKOUT"
    git clone --depth 1 "$CORE_REPO" "$CORE_CHECKOUT" 2>/dev/null
    git -C "$CORE_CHECKOUT" fetch --depth 1 origin "$CORE_SHA" 2>/dev/null || \
        git -C "$CORE_CHECKOUT" fetch origin "$CORE_SHA" 2>/dev/null
    git -C "$CORE_CHECKOUT" checkout "$CORE_SHA"
fi

CORE_CHECKOUT_NORM=$(normalize_path "$CORE_CHECKOUT")
echo "Core checkout: $CORE_CHECKOUT @ $(git -C "$CORE_CHECKOUT" rev-parse --short HEAD)"

# --- 2. Discover all openbim-* crate names in core ---
CRATES_DIR="$CORE_CHECKOUT/crates"
if [ ! -d "$CRATES_DIR" ]; then
    echo "ERROR: $CRATES_DIR not found"
    exit 1
fi

PATCH_CRATES=()
for d in "$CRATES_DIR"/*/; do
    ctoml="$d/Cargo.toml"
    if [ -f "$ctoml" ]; then
        name=$(grep -m1 '^name = ' "$ctoml" | sed 's/name = "\(.*\)"/\1/')
        if [ -n "$name" ]; then
            PATCH_CRATES+=("$name")
        fi
    fi
done

echo "Patch crates: ${#PATCH_CRATES[@]} found"

# --- 3. Generate [patch] section ---
generate_patch() {
    local target_toml="$1"
    {
        echo ''
        echo "[patch.\"$CORE_URL\"]"
        for c in "${PATCH_CRATES[@]}"; do
            short="${c#openbim-}"
            printf '%s = { path = "%s/crates/%s" }\n' "$c" "$CORE_CHECKOUT_NORM" "$short"
        done
    } >> "$target_toml"
}

restore_toml() {
    local target_toml="$1"
    git -C "$(dirname "$(dirname "$target_toml")")" checkout -- "$target_toml" 2>/dev/null || true
}

# --- 4. Consumer repos ---
# Only workspace-root Cargo.toml is patched (cargo ignores [patch] in members)
REVIT_ALIKE="/c/Users/wilfr/hermes-first-task/wt-g-m02"
EDESIGL_ALIKE="/c/Users/wilfr/hermes-first-task/edesign-alike"

CONSUMERS=(
    "revit-alike:$REVIT_ALIKE:Cargo.toml"
    "edesign-alike:$EDESIGL_ALIKE:Cargo.toml"
)

FAILED=0

for consumer in "${CONSUMERS[@]}"; do
    IFS=':' read -r name repo root_toml <<< "$consumer"
    echo ""
    echo "=========================================="
    echo "CONSUMER: $name ($repo)"
    echo "=========================================="

    # Save original Cargo.toml content
    cp "$repo/$root_toml" "$repo/$root_toml.gate.bak" 2>/dev/null || true

    # Patch workspace-root Cargo.toml
    generate_patch "$repo/$root_toml"

    # Run tests
    set +e
    echo "--- cargo test --workspace --release ---"
    (cd "$repo" && cargo test --workspace --release 2>&1)
    TEST_RC=$?
    set -e

    if [ $TEST_RC -ne 0 ]; then
        echo "FAIL: cargo test in $name (exit $TEST_RC)"
        FAILED=1
    else
        echo "PASS: cargo test in $name"
    fi

    # Restore original Cargo.toml files
    cp "$repo/$root_toml.gate.bak" "$repo/$root_toml" 2>/dev/null || true
    rm -f "$repo/$root_toml.gate.bak" 2>/dev/null || true
done

echo ""
echo "=========================================="
if [ $FAILED -eq 0 ]; then
    echo "RESULT: ALL CONSUMERS PASSED"
    exit 0
else
    echo "RESULT: ONE OR MORE CONSUMERS FAILED"
    exit 1
fi
