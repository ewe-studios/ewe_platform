#!/bin/bash
# Batch generate ALL GCP API providers
# Usage: ./scripts/gen_all_gcp.sh

set -e

PROVIDER_DIR="artefacts/cloud_providers/gcp"
OUTPUT_DIR="backends/foundation_deployment/src/providers/gcp"

# Get list of all APIs (skip . and ..)
APIS=$(ls "$PROVIDER_DIR" | grep -v "^\." | sort)

TOTAL=$(echo "$APIS" | wc -l)
COUNT=0
SUCCESS=0
FAILED=0
FAILED_APIS=""

echo "=== GCP Full Batch Generation ==="
echo "Total APIs to generate: $TOTAL"
echo ""

# First, clean existing generated sub-providers (keep shared, fetch, provider, resources)
rm -rf "$OUTPUT_DIR"/[a-z]* 2>/dev/null || true

for API in $APIS; do
    COUNT=$((COUNT + 1))
    echo "[$COUNT/$TOTAL] Generating gcp/$API..."

    if cargo run --quiet --bin ewe_platform -- gen_api generate "gcp/$API" --features 2>&1 | grep -q "Generation Complete"; then
        echo "  [OK] gcp/$API"
        SUCCESS=$((SUCCESS + 1))
    else
        echo "  [FAIL] gcp/$API"
        FAILED=$((FAILED + 1))
        FAILED_APIS="$FAILED_APIS $API"
    fi

    # Periodic cargo check every 20 APIs to catch errors early
    if [ $((COUNT % 20)) -eq 0 ]; then
        echo "  [Checkpoint $COUNT] Running cargo check..."
        if cargo check --features gcp --package foundation_deployment --quiet 2>/dev/null; then
            echo "  [OK] Compilation check passed"
        else
            echo "  [WARN] Compilation has errors, continuing..."
        fi
    fi
done

echo ""
echo "=== Generation Complete ==="
echo "Successful: $SUCCESS / $TOTAL"
echo "Failed: $FAILED"
if [ $FAILED -gt 0 ]; then
    echo "Failed APIs:$FAILED_APIS"
fi

# Final compilation check
echo ""
echo "=== Final Compilation Check ==="
cargo check --features gcp --package foundation_deployment 2>&1 | tail -5
