/**
 * Test 70: Parameterized Query Tests
 *
 * Tests runtime string parameters in query functions.
 *
 * Usage:
 *   cargo run -p covenant-cli -- compile examples/70-parameterized-query.cov -o examples/70-parameterized-query.wasm
 *   deno run --allow-read examples/70-test.ts
 */

import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

const runner = new CovenantQueryRunner();

console.log("=== Test 70: Parameterized Queries ===\n");

// Load the compiled WASM module
try {
  await runner.load("./examples/70-parameterized-query.wasm");
  console.log("WASM module loaded successfully\n");
} catch (e) {
  console.error("Failed to load WASM:", e);
  Deno.exit(1);
}

// List available exports
console.log("Available exports:", runner.listExports());
console.log();

// Test cov_alloc is exported
const exports = runner.listExports();
console.log("Test: cov_alloc is exported");
if (exports.includes("cov_alloc")) {
  console.log("  PASS: cov_alloc found in exports\n");
} else {
  console.log("  FAIL: cov_alloc not found\n");
}

// Test basic allocation
console.log("Test: Basic memory allocation");
const ptr1 = runner.alloc(100);
const ptr2 = runner.alloc(100);
console.log(`  Allocated 100 bytes at: ${ptr1}`);
console.log(`  Allocated 100 bytes at: ${ptr2}`);
if (ptr2 > ptr1) {
  console.log("  PASS: Second allocation is after first\n");
} else {
  console.log("  FAIL: Allocation not advancing\n");
}

// Test allocString
console.log("Test: allocString creates fat pointer");
const testStr = "Hello, World!";
const fatPtr = runner.allocString(testStr);
const extractedPtr = Number(fatPtr >> 32n);
const extractedLen = Number(fatPtr & 0xFFFFFFFFn);
console.log(`  Input string: "${testStr}"`);
console.log(`  Fat pointer: ${fatPtr}`);
console.log(`  Extracted ptr: ${extractedPtr}, len: ${extractedLen}`);
if (extractedLen === testStr.length) {
  console.log("  PASS: Length matches\n");
} else {
  console.log(`  FAIL: Expected len ${testStr.length}, got ${extractedLen}\n`);
}

// Test search_content with "Hello"
console.log("Test: search_content('Hello')");
try {
  const results = runner.queryWithString("search_content", "Hello");
  const nodes = runner.getQueryResultNodes(results);
  console.log(`  Found ${nodes.length} nodes`);
  nodes.forEach((node) => {
    console.log(`    - ${node.id}`);
  });
  // Should find docs.hello (contains "Hello World")
  const foundHello = nodes.some((n) => n.id === "docs.hello");
  if (foundHello && nodes.length >= 1) {
    console.log("  PASS: Found docs.hello\n");
  } else {
    console.log("  FAIL: Expected to find docs.hello\n");
  }
} catch (e) {
  console.log(`  FAIL: Error - ${e}\n`);
}

// Test search_content with "World"
console.log("Test: search_content('World')");
try {
  const results = runner.queryWithString("search_content", "World");
  const nodes = runner.getQueryResultNodes(results);
  console.log(`  Found ${nodes.length} nodes`);
  nodes.forEach((node) => {
    console.log(`    - ${node.id}`);
  });
  // Should find docs.hello and docs.goodbye (both contain "World")
  const foundBoth =
    nodes.some((n) => n.id === "docs.hello") &&
    nodes.some((n) => n.id === "docs.goodbye");
  if (foundBoth && nodes.length >= 2) {
    console.log("  PASS: Found both docs.hello and docs.goodbye\n");
  } else {
    console.log("  FAIL: Expected to find both World documents\n");
  }
} catch (e) {
  console.log(`  FAIL: Error - ${e}\n`);
}

// Test search_content with "advanced"
console.log("Test: search_content('advanced')");
try {
  const results = runner.queryWithString("search_content", "advanced");
  const nodes = runner.getQueryResultNodes(results);
  console.log(`  Found ${nodes.length} nodes`);
  nodes.forEach((node) => {
    console.log(`    - ${node.id}`);
  });
  const foundAdvanced = nodes.some((n) => n.id === "docs.advanced");
  if (foundAdvanced && nodes.length === 1) {
    console.log("  PASS: Found only docs.advanced\n");
  } else {
    console.log("  FAIL: Expected to find only docs.advanced\n");
  }
} catch (e) {
  console.log(`  FAIL: Error - ${e}\n`);
}

// Test search_content with non-existent term
console.log("Test: search_content('nonexistent_xyz')");
try {
  const results = runner.queryWithString("search_content", "nonexistent_xyz");
  const nodes = runner.getQueryResultNodes(results);
  console.log(`  Found ${nodes.length} nodes`);
  if (nodes.length === 0) {
    console.log("  PASS: No results for non-existent term\n");
  } else {
    console.log("  FAIL: Expected 0 results\n");
  }
} catch (e) {
  console.log(`  FAIL: Error - ${e}\n`);
}

// Test find_by_id
console.log("Test: find_by_id('docs.tutorial')");
try {
  const results = runner.queryWithString("find_by_id", "docs.tutorial");
  const nodes = runner.getQueryResultNodes(results);
  console.log(`  Found ${nodes.length} nodes`);
  nodes.forEach((node) => {
    console.log(`    - ${node.id}`);
  });
  if (nodes.length === 1 && nodes[0].id === "docs.tutorial") {
    console.log("  PASS: Found exact match\n");
  } else {
    console.log("  FAIL: Expected exactly docs.tutorial\n");
  }
} catch (e) {
  console.log(`  FAIL: Error - ${e}\n`);
}

// Test find_by_kind
console.log("Test: find_by_kind('data')");
try {
  const results = runner.queryWithString("find_by_kind", "data");
  const nodes = runner.getQueryResultNodes(results);
  console.log(`  Found ${nodes.length} nodes`);
  nodes.forEach((node) => {
    console.log(`    - ${node.id} (kind: ${node.kind})`);
  });
  // Should find all 4 data nodes
  const allData = nodes.every((n) => n.kind === "data");
  if (nodes.length === 4 && allData) {
    console.log("  PASS: Found all 4 data nodes\n");
  } else {
    console.log(`  FAIL: Expected 4 data nodes, got ${nodes.length}\n`);
  }
} catch (e) {
  console.log(`  FAIL: Error - ${e}\n`);
}

console.log("=== Test 70 Complete ===");
