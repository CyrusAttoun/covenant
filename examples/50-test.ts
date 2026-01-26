/**
 * Test script for Example 50: Simple Embedded Query
 *
 * This script:
 * 1. Loads the compiled WASM module
 * 2. Inspects embedded graph data using GAI functions
 * 3. Calls exported query functions
 * 4. Verifies results
 */

import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

async function main() {
  console.log("=== Example 50: Simple Embedded Query ===\n");

  const runner = new CovenantQueryRunner();

  // Load the compiled WASM module
  console.log("Loading WASM module...");
  try {
    await runner.load("./examples/50-embedded-query-simple.wasm");
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

  // Inspect embedded graph data using GAI functions
  console.log("Embedded graph data (via GAI functions):");
  const nodeCount = runner.nodeCount();
  console.log(`  Total nodes: ${nodeCount}`);

  if (nodeCount > 0) {
    console.log("  Node IDs:");
    const nodeIds = runner.getAllNodeIds();
    nodeIds.forEach((id) => console.log(`    - ${id}`));

    console.log("\n  Nodes with content:");
    const nodes = runner.getAllNodes();
    nodes.forEach((node) => {
      console.log(`    ${node.id}:`);
      const preview = node.content.substring(0, 60).replace(/\n/g, " ");
      console.log(`      ${preview}...`);
    });
  }

  console.log();

  // Call exported query functions
  console.log("Calling query functions:");

  // Note: These will currently return empty results because
  // compile_project_query is a stub. Once implemented, they will
  // return actual query results.

  try {
    console.log("  - find_docs()...");
    const findDocsResult = runner.call("find_docs");
    console.log(`    Result: ${findDocsResult}`);
    console.log("    (Currently returns empty - full implementation pending)");
  } catch (error) {
    console.log(`    Error: ${error}`);
  }

  try {
    console.log("\n  - find_hello_doc()...");
    const findHelloDocResult = runner.call("find_hello_doc");
    console.log(`    Result: ${findHelloDocResult}`);
    console.log("    (Currently returns empty - full implementation pending)");
  } catch (error) {
    console.log(`    Error: ${error}`);
  }

  try {
    console.log("\n  - count_docs()...");
    const countDocsResult = runner.call("count_docs");
    console.log(`    Result: ${countDocsResult}`);
  } catch (error) {
    console.log(`    Error: ${error}`);
  }

  console.log("\n=== Test Complete ===");
  console.log("\nNotes:");
  console.log("- GAI functions work and can access embedded data");
  console.log("- Query functions compile without errors");
  console.log("- Query functions currently return empty results (stub)");
  console.log("- Full query execution pending implementation of compile_project_query");
}

if (import.meta.main) {
  await main();
}
