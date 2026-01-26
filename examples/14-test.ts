/**
 * Test script for Example 14: Project Queries
 *
 * This script demonstrates querying the symbol graph (code metadata)
 * using target="project" queries.
 */

import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

async function main() {
  console.log("=== Example 14: Project Queries ===\n");

  const runner = new CovenantQueryRunner();

  // Load the compiled WASM module
  console.log("Loading WASM module...");
  try {
    await runner.load("./examples/14-project-queries.wasm");
    console.log("✓ Module loaded successfully\n");
  } catch (error) {
    console.error("✗ Failed to load WASM module:");
    console.error(error);
    Deno.exit(1);
  }

  // List all exports
  console.log("Exported functions:");
  const exports = runner.listExports();
  exports.forEach((name) => console.log(`  - ${name}`));
  console.log();

  // Note: Symbol queries are different from data queries
  // Symbol queries inspect the code's own metadata (functions, types, effects)
  // This example shows the intended usage, but full implementation requires
  // embedding the symbol graph in WASM (not just the data graph)

  console.log("Symbol Graph Queries (metadata about code):\n");

  console.log("1. find_db_functions()");
  console.log("   Query: Find all functions with 'database' effect");
  console.log("   (Pending: symbol graph embedding)\n");

  console.log("2. find_callers('some_function')");
  console.log("   Query: Find all functions that call 'some_function'");
  console.log("   (Pending: symbol graph embedding with bidirectional refs)\n");

  console.log("3. find_dead_code()");
  console.log("   Query: Find uncalled, non-exported functions");
  console.log("   (Pending: symbol graph embedding)\n");

  console.log("4. find_urls()");
  console.log("   Query: Find all string literals matching https?://.*");
  console.log("   (Pending: symbol graph embedding with literal tracking)\n");

  console.log("5. find_untested_requirements()");
  console.log("   Query: Find requirements not covered by tests");
  console.log("   (Pending: symbol graph embedding with test coverage)\n");

  console.log("=== Test Complete ===");
  console.log("\nNotes:");
  console.log("- This example demonstrates SYMBOL queries (code metadata)");
  console.log("- Requires --embed-symbols=full compilation flag");
  console.log("- Symbol graph is separate from data graph (kind=\"data\")");
  console.log("- Full implementation pending");
}

if (import.meta.main) {
  await main();
}
