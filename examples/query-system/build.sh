#!/bin/bash
#
# Build script for Covenant Query System examples
#
# This script:
# 1. Compiles doc-ingestion.cov
# 2. Runs ingestion to generate data .cov files
# 3. Concatenates generated data with each query example
# 4. Compiles each combined file to WASM

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Covenant Query System Build ==="
echo ""

# Ensure output directory exists
mkdir -p output

# Step 1: Compile doc-ingestion
echo "Step 1: Compiling doc-ingestion.cov..."
../../target/release/covenant compile doc-ingestion.cov -o output/doc-ingestion.wasm
echo "  Created: output/doc-ingestion.wasm"
echo ""

# Step 2: Run ingestion to generate data files
echo "Step 2: Running doc ingestion..."
deno run --allow-read --allow-write run-ingestion.ts
echo ""

# Step 3: Collect all generated data files
echo "Step 3: Collecting generated data files..."
# Use find to also match hidden files (starting with .)
DATA_FILES=$(find output -maxdepth 1 -name "*.cov" -type f 2>/dev/null | sort || true)

if [ -z "$DATA_FILES" ]; then
  echo "  Warning: No .cov files found in output/"
  echo "  Creating empty placeholder..."
  echo "// No data ingested" > output/_empty.cov
  DATA_FILES="output/_empty.cov"
fi

echo "  Data files:"
for f in $DATA_FILES; do
  echo "    - $f"
done
echo ""

# Step 4: Build each query example
echo "Step 4: Building query examples..."

for example in embedded-query.cov parameterized-query.cov relation-traversal.cov rag-query.cov; do
  if [ -f "$example" ]; then
    name=$(basename "$example" .cov)
    echo "  Building $name..."

    # Concatenate data files + example into temp file
    cat $DATA_FILES "$example" > "/tmp/${name}-combined.cov"

    # Compile
    ../../target/release/covenant compile "/tmp/${name}-combined.cov" -o "output/${name}.wasm"
    echo "    Created: output/${name}.wasm"
  else
    echo "  Skipping $example (not found)"
  fi
done

echo ""
echo "=== Build Complete ==="
echo ""
echo "To query the examples, run:"
echo "  deno run --allow-read query-repl.ts"
echo ""
echo "Then load a module:"
echo "  :load output/rag-query.wasm"
echo "  :query effects"
