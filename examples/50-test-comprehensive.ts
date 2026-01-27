/**
 * Comprehensive test for Example 50: Embedded Query System
 *
 * Tests:
 * 1. Basic query execution (find all data nodes)
 * 2. WHERE clause with equals (find specific node)
 * 3. AND conditions
 * 4. LIMIT clause
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
    console.log(passed ? `✓ ${name}` : `✗ ${name}`);
  } catch (error) {
    results.push({ name, passed: false, error: String(error) });
    console.log(`✗ ${name}: ${error}`);
  }
}

async function main() {
  console.log("=== Comprehensive Query System Tests ===\n");

  const runner = new CovenantQueryRunner();
  await runner.load("./examples/50-embedded-query-simple.wasm");

  const memory = (runner as any).memory;

  // Helper to unpack fat pointer
  function unpackFatPtr(fatPtr: bigint): { ptr: number; count: number } {
    const ptr = Number(fatPtr >> 32n);
    const count = Number(fatPtr & 0xFFFFFFFFn);
    return { ptr, count };
  }

  // Helper to read node indices from result array
  function readNodeIndices(ptr: number, count: number): number[] {
    if (count === 0) return [];
    const view = new DataView(memory.buffer, ptr, count * 4);
    const indices: number[] = [];
    for (let i = 0; i < count; i++) {
      indices.push(view.getUint32(i * 4, true));
    }
    return indices;
  }

  // Helper to get node kind
  function getNodeKind(idx: number): string {
    const kindFatPtr = runner.call("cov_get_node_kind", idx) as bigint;
    const { ptr, count } = unpackFatPtr(kindFatPtr);
    const view = new Uint8Array(memory.buffer, ptr, count);
    return new TextDecoder().decode(view);
  }

  // Helper to get node ID
  function getNodeId(idx: number): string {
    const idFatPtr = runner.call("cov_get_node_id", idx) as bigint;
    const { ptr, count } = unpackFatPtr(idFatPtr);
    const view = new Uint8Array(memory.buffer, ptr, count);
    return new TextDecoder().decode(view);
  }

  console.log("--- Test 1: find_docs (all data nodes) ---");
  {
    const result = runner.call("find_docs") as bigint;
    const { ptr, count } = unpackFatPtr(result);
    const indices = readNodeIndices(ptr, count);

    console.log(`  Result count: ${count}`);
    console.log(`  Node indices: [${indices.join(", ")}]`);

    test("find_docs returns 2 nodes", () => count === 2);
    test("find_docs returns nodes 0 and 1", () =>
      indices.length === 2 && indices[0] === 0 && indices[1] === 1);

    if (count === 2) {
      const kind0 = getNodeKind(indices[0]);
      const kind1 = getNodeKind(indices[1]);
      console.log(`  Node 0 kind: "${kind0}"`);
      console.log(`  Node 1 kind: "${kind1}"`);
      test("find_docs returns only data nodes", () => kind0 === "data" && kind1 === "data");
    }
  }

  console.log("\n--- Test 2: find_hello_doc (specific node with AND) ---");
  {
    const result = runner.call("find_hello_doc") as bigint;
    const { ptr, count } = unpackFatPtr(result);
    const indices = readNodeIndices(ptr, count);

    console.log(`  Result count: ${count}`);
    console.log(`  Node indices: [${indices.join(", ")}]`);

    test("find_hello_doc returns 1 node (with LIMIT)", () => count === 1);
    test("find_hello_doc returns node 0", () => indices.length === 1 && indices[0] === 0);

    if (count === 1) {
      const id = getNodeId(indices[0]);
      const kind = getNodeKind(indices[0]);
      console.log(`  Node ID: "${id}"`);
      console.log(`  Node kind: "${kind}"`);
      test("find_hello_doc returns docs.hello", () => id === "docs.hello");
      test("find_hello_doc returns a data node", () => kind === "data");
    }
  }

  console.log("\n--- Test 3: GAI function correctness ---");
  {
    const nodeCount = runner.call("cov_node_count") as number;
    test("Node count is 6", () => nodeCount === 6);

    const dataCount = [0, 1, 2, 3, 4, 5].filter(i => getNodeKind(i) === "data").length;
    test("Exactly 2 nodes have kind=data", () => dataCount === 2);

    const fnCount = [0, 1, 2, 3, 4, 5].filter(i => getNodeKind(i) === "fn").length;
    test("Exactly 4 nodes have kind=fn", () => fnCount === 4);
  }

  console.log("\n--- Summary ---");
  const passed = results.filter(r => r.passed).length;
  const total = results.length;
  console.log(`${passed}/${total} tests passed`);

  if (passed === total) {
    console.log("\n✅ All tests passed!");
  } else {
    console.log("\n❌ Some tests failed:");
    results.filter(r => !r.passed).forEach(r => {
      console.log(`  - ${r.name}${r.error ? `: ${r.error}` : ""}`);
    });
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
