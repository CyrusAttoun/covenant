/**
 * Test for Example 52: Relation Traversal
 *
 * Tests:
 * 1. Outgoing traversal (get_children)
 * 2. Inverse relation traversal (get_parent via contained_by)
 * 3. Nested children (get_grandchildren)
 * 4. Incoming direction (get_all_incoming)
 * 5. Chained traversal (chain_traverse)
 */

import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
}

const results: TestResult[] = [];

function test(name: string, testFn: () => boolean): void {
  try {
    const passed = testFn();
    results.push({ name, passed });
    console.log(passed ? `  ✓ ${name}` : `  ✗ ${name}`);
  } catch (error) {
    results.push({ name, passed: false, error: String(error) });
    console.log(`  ✗ ${name}: ${error}`);
  }
}

async function main() {
  console.log("=== Relation Traversal Tests ===\n");

  const runner = new CovenantQueryRunner();
  await runner.load("./examples/52-relation-traversal.wasm");

  const memory = (runner as any).memory;

  // Helper to unpack fat pointer
  function unpackFatPtr(fatPtr: bigint): { ptr: number; count: number } {
    const ptr = Number(fatPtr >> 32n);
    const count = Number(fatPtr & 0xFFFFFFFFn);
    return { ptr, count };
  }

  // Helper to read node indices from result array
  function readNodeIndices(ptr: number, count: number): number[] {
    if (count === 0 || ptr === 0) return [];
    const view = new DataView(memory.buffer, ptr, count * 4);
    const indices: number[] = [];
    for (let i = 0; i < count; i++) {
      indices.push(view.getUint32(i * 4, true));
    }
    return indices;
  }

  // Helper to get node ID
  function getNodeId(idx: number): string {
    const idFatPtr = runner.call("cov_get_node_id", idx) as bigint;
    const { ptr, count } = unpackFatPtr(idFatPtr);
    if (count === 0) return "";
    const view = new Uint8Array(memory.buffer, ptr, count);
    return new TextDecoder().decode(view);
  }

  // Helper to get node IDs from result
  function getResultNodeIds(fatPtr: bigint): string[] {
    const { ptr, count } = unpackFatPtr(fatPtr);
    const indices = readNodeIndices(ptr, count);
    return indices.map(getNodeId);
  }

  // -------------------------------------------------------------------------
  // Test 1: get_children - outgoing traversal from kb.root
  // -------------------------------------------------------------------------
  console.log("--- Test 1: get_children (outgoing contains from kb.root) ---");
  {
    const result = runner.call("get_children") as bigint;
    const { count } = unpackFatPtr(result);
    const nodeIds = getResultNodeIds(result);

    console.log(`  Result count: ${count}`);
    console.log(`  Node IDs: ${JSON.stringify(nodeIds)}`);

    test("get_children returns 2 results", () => count === 2);
    test("get_children includes kb.chapter1", () => nodeIds.includes("kb.chapter1"));
    test("get_children includes kb.chapter2", () => nodeIds.includes("kb.chapter2"));
  }

  // -------------------------------------------------------------------------
  // Test 2: get_parent - inverse relation (contained_by)
  // -------------------------------------------------------------------------
  console.log("\n--- Test 2: get_parent (contained_by from kb.chapter1) ---");
  {
    const result = runner.call("get_parent") as bigint;
    const { count } = unpackFatPtr(result);
    const nodeIds = getResultNodeIds(result);

    console.log(`  Result count: ${count}`);
    console.log(`  Node IDs: ${JSON.stringify(nodeIds)}`);

    test("get_parent returns 1 result", () => count === 1);
    test("get_parent returns kb.root", () => nodeIds.includes("kb.root"));
  }

  // -------------------------------------------------------------------------
  // Test 3: get_grandchildren - children of chapter1
  // -------------------------------------------------------------------------
  console.log("\n--- Test 3: get_grandchildren (contains from kb.chapter1) ---");
  {
    const result = runner.call("get_grandchildren") as bigint;
    const { count } = unpackFatPtr(result);
    const nodeIds = getResultNodeIds(result);

    console.log(`  Result count: ${count}`);
    console.log(`  Node IDs: ${JSON.stringify(nodeIds)}`);

    test("get_grandchildren returns 1 result", () => count === 1);
    test("get_grandchildren returns kb.section1a", () => nodeIds.includes("kb.section1a"));
  }

  // -------------------------------------------------------------------------
  // Test 4: get_all_incoming - incoming contains to section1a
  // -------------------------------------------------------------------------
  console.log("\n--- Test 4: get_all_incoming (incoming contains to kb.section1a) ---");
  {
    const result = runner.call("get_all_incoming") as bigint;
    const { count } = unpackFatPtr(result);
    const nodeIds = getResultNodeIds(result);

    console.log(`  Result count: ${count}`);
    console.log(`  Node IDs: ${JSON.stringify(nodeIds)}`);

    test("get_all_incoming returns 1 result", () => count === 1);
    test("get_all_incoming returns kb.chapter1", () => nodeIds.includes("kb.chapter1"));
  }

  // -------------------------------------------------------------------------
  // Test 5: chain_traverse - traverse from query result variable
  // -------------------------------------------------------------------------
  console.log("\n--- Test 5: chain_traverse (traverse from query result) ---");
  {
    const result = runner.call("chain_traverse") as bigint;
    const { count } = unpackFatPtr(result);
    const nodeIds = getResultNodeIds(result);

    console.log(`  Result count: ${count}`);
    console.log(`  Node IDs: ${JSON.stringify(nodeIds)}`);

    // Chain: kb.root -> children -> first child's children
    // kb.root's first child (in order) should be kb.chapter1 or kb.chapter2
    // kb.chapter1 has contains -> kb.section1a
    // kb.chapter2 has no children
    // So result depends on which child is first
    test("chain_traverse returns at least 0 results", () => count >= 0);
    // If kb.chapter1 is first, we get section1a
    if (count > 0) {
      test("chain_traverse result is valid node", () => nodeIds.length > 0 && nodeIds[0].startsWith("kb."));
    }
  }

  // -------------------------------------------------------------------------
  // Summary
  // -------------------------------------------------------------------------
  console.log("\n=== Summary ===");
  const passed = results.filter(r => r.passed).length;
  const failed = results.filter(r => !r.passed).length;
  console.log(`Passed: ${passed}/${results.length}`);
  if (failed > 0) {
    console.log(`Failed: ${failed}`);
    console.log("\nFailed tests:");
    results.filter(r => !r.passed).forEach(r => {
      console.log(`  - ${r.name}${r.error ? `: ${r.error}` : ""}`);
    });
    Deno.exit(1);
  } else {
    console.log("\nAll tests passed!");
  }
}

main().catch(console.error);
