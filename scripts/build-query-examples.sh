#!/usr/bin/env bash
#
# Build script for query examples
#
# This script compiles the query example programs to WASM:
# - Example 50: Simple embedded query
# - Example 20: Knowledge base traversal
# - Example 14: Project queries (symbol graph)

set -e  # Exit on error

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Building Query Examples ===${NC}\n"

# Build the compiler first
echo -e "${YELLOW}Building Covenant compiler...${NC}"
cargo build --release -p covenant-cli
echo -e "${GREEN}✓ Compiler built${NC}\n"

COVENANT="./target/release/covenant"

# Helper function to compile an example
compile_example() {
    local example_num=$1
    local example_name=$2
    local input="./examples/${example_num}-${example_name}.cov"
    local output="./examples/${example_num}-${example_name}.wasm"

    echo -e "${YELLOW}Compiling Example ${example_num}: ${example_name}${NC}"
    echo "  Input:  $input"
    echo "  Output: $output"

    if [ ! -f "$input" ]; then
        echo -e "${RED}  ✗ Input file not found${NC}"
        return 1
    fi

    if $COVENANT compile "$input" --output "$output" --target deno; then
        local size=$(stat -c%s "$output" 2>/dev/null || stat -f%z "$output" 2>/dev/null)
        echo -e "${GREEN}  ✓ Compiled successfully (${size} bytes)${NC}\n"
    else
        echo -e "${RED}  ✗ Compilation failed${NC}\n"
        return 1
    fi
}

# Compile examples
compile_example "50" "embedded-query-simple" || true
compile_example "20" "knowledge-base" || true
compile_example "14" "project-queries" || true

echo -e "${GREEN}=== Build Complete ===${NC}\n"

# List generated files
echo "Generated WASM files:"
ls -lh ./examples/*.wasm 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'  || echo "  (none)"
echo ""

echo "To run tests:"
echo "  deno run --allow-read examples/50-test.ts"
echo "  deno run --allow-read examples/20-test.ts"
echo "  deno run --allow-read examples/14-test.ts"
