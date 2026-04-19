#!/bin/bash
# Batch generate all GCP API providers
# Usage: ./scripts/gen_gcp_apis.sh [--batch-size N] [--start-after API_NAME]

set -e

BATCH_SIZE=${1:-50}
START_AFTER=${2:-""}

PROVIDER_DIR="artefacts/cloud_providers/gcp"
OUTPUT_DIR="backends/foundation_deployment/src/providers/gcp"

# Get list of all APIs
APIS=$(ls "$PROVIDER_DIR" | sort)

# Skip APIs before start point if specified
if [ -n "$START_AFTER" ]; then
    APIS=$(echo "$APIS" | sed "1,/^[[:space:]]*${START_AFTER}[[:space:]]*$/d")
fi

TOTAL=$(echo "$APIS" | wc -l)
COUNT=0
SUCCESS=0
FAILED=0

echo "=== GCP Batch Generation ==="
echo "Total APIs to generate: $TOTAL"
echo "Batch size: $BATCH_SIZE"
echo "Starting after: ${START_AFTER:-none (starting from beginning)}"
echo ""

for API in $APIS; do
    COUNT=$((COUNT + 1))
    echo "[$COUNT/$TOTAL] Generating $API..."

    if cargo run --bin ewe_platform --quiet -- gen_api generate "gcp/$API" --features 2>&1; then
        echo "  [OK] $API"
        SUCCESS=$((SUCCESS + 1))
    else
        echo "  [FAIL] $API"
        FAILED=$((FAILED + 1))
    fi

    # Periodic cargo clean to free memory
    if [ $((COUNT % BATCH_SIZE)) -eq 0 ]; then
        echo "  [BATCH COMPLETE] Running cargo check..."
        cargo check --features gcp --package foundation_deployment --quiet 2>/dev/null || true
    fi
done

echo ""
echo "=== Generation Complete ==="
echo "Successful: $SUCCESS"
echo "Failed: $FAILED"
echo "Total: $COUNT"
