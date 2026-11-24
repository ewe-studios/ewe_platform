#!/usr/bin/env bash
# Generate Google Cloud providers in batches. Arguments are the batch size and last completed provider.
set -euo pipefail
batch_size=${1:-50}
start_after=${2:-}
if [[ ! $batch_size =~ ^[1-9][0-9]*$ ]]; then
    echo "The batch size must be a positive integer." >&2
    exit 2
fi

generator=(cargo run --locked --profile uat --bin ewe_platform -- task run codegen:generate)
# Validate all inputs before changing any generated source.
"${generator[@]}" gcp --features --dry-run
export LC_ALL=C
specs=()
for directory in artefacts/cloud_providers/gcp/*/; do
    [[ -d $directory ]] || continue
    name=${directory%/}
    name=${name##*/}
    if [[ -n $start_after ]]; then
        if [[ $name == "$start_after" ]]; then start_after=; fi
        continue
    fi
    specs+=("$name")
done
if [[ -n $start_after ]]; then
    echo "Unknown starting provider: $start_after" >&2
    exit 2
fi
count=0
for spec in "${specs[@]}"; do
    "${generator[@]}" "gcp/$spec" --features
    count=$((count + 1))
    if ((count % batch_size == 0)); then
        cargo check --package foundation_deployment --features gcp
    fi
done
if ((count % batch_size != 0)); then
    cargo check --package foundation_deployment --features gcp
fi
