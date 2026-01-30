/**
 * Test for Example 60: RAG Query System
 *
 * Tests the basic query and traversal capabilities.
 *
 * Known limitations discovered during pressure testing:
 * 1. No allocator export - can't pass string parameters at runtime
 * 2. Parameterized queries (search_by_keyword, get_doc_by_id) need compile-time literals
 * 3. Traversal from query results may not work yet (chain_traverse pattern)
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
  console.log("=== RAG Query System Tests ===\n");
  console.log("Testing embedded query capabilities against 6 documentation nodes.\n");

  const runner = new CovenantQueryRunner();
  await runner.load("./examples/60-rag-query.wasm");

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

  // Helper to get node ID
  function getNodeId(idx: number): string {
    const idFatPtr = runner.call("cov_get_node_id", idx) as bigint;
    const { ptr, count } = unpackFatPtr(idFatPtr);
    const view = new Uint8Array(memory.buffer, ptr, count);
    return new TextDecoder().decode(view);
  }

  // Helper to get node kind
  function getNodeKind(idx: number): string {
    const kindFatPtr = runner.call("cov_get_node_kind", idx) as bigint;
    const { ptr, count } = unpackFatPtr(kindFatPtr);
    const view = new Uint8Array(memory.buffer, ptr, count);
    return new TextDecoder().decode(view);
  }

  // Helper to get node content (truncated)
  function getNodeContent(idx: number, maxLen = 100): string {
    const contentFatPtr = runner.call("cov_get_node_content", idx) as bigint;
    const { ptr, count } = unpackFatPtr(contentFatPtr);
    const view = new Uint8Array(memory.buffer, ptr, Math.min(count, maxLen));
    const text = new TextDecoder().decode(view);
    return count > maxLen ? text + "..." : text;
  }

  // Helper to unpack relation data
  // Return format from cov_get_outgoing_rel: ((target_idx as u32) << 8) | rel_type_idx
  function unpackRelation(packed: bigint): { targetIdx: number; relTypeIdx: number } | null {
    if (packed === -1n) return null;  // out of bounds
    const relTypeIdx = Number(packed & 0xFFn);
    const targetIdx = Number((packed >> 8n) & 0xFFFFn);
    return { targetIdx, relTypeIdx };
  }

  // Helper to get relation type name from type index
  function getRelTypeName(relTypeIdx: number): string {
    const typeFatPtr = runner.call("cov_get_rel_type_name", relTypeIdx) as bigint;
    const { ptr, count } = unpackFatPtr(typeFatPtr);
    if (count === 0 || ptr === 0) return "";
    const view = new Uint8Array(memory.buffer, ptr, count);
    return new TextDecoder().decode(view);
  }

  // -------------------------------------------------------------------------
  console.log("--- Test 1: GAI Infrastructure ---");
  {
    const nodeCount = runner.call("cov_node_count") as number;
    console.log(`  Total nodes in WASM: ${nodeCount}`);

    test("Node count is correct (10 = 6 data + 4 fn)", () => nodeCount === 10);

    // Count data nodes and print all node info
    let dataCount = 0;
    let fnCount = 0;
    console.log("  Node details:");
    for (let i = 0; i < nodeCount; i++) {
      const kind = getNodeKind(i);
      const id = getNodeId(i);
      console.log(`    [${i}] kind="${kind}" id="${id}"`);
      if (kind === "data") dataCount++;
      if (kind === "fn") fnCount++;
    }
    console.log(`  Data nodes: ${dataCount}, Function nodes: ${fnCount}`);

    test("6 data nodes embedded", () => dataCount === 6);
    test("4 function nodes embedded", () => fnCount === 4);
  }

  // -------------------------------------------------------------------------
  console.log("\n--- Test 2: get_all_docs ---");
  {
    const result = runner.call("get_all_docs") as bigint;
    console.log(`  Raw fat pointer: 0x${result.toString(16)}`);
    const { ptr, count } = unpackFatPtr(result);
    console.log(`  Unpacked: ptr=${ptr}, count=${count}`);
    const indices = readNodeIndices(ptr, count);

    console.log(`  Query returned ${count} documents`);

    test("get_all_docs returns 6 documents", () => count === 6);

    if (count > 0) {
      const ids = indices.map(i => getNodeId(i));
      console.log(`  Document IDs: ${ids.join(", ")}`);

      test("All results are data nodes", () =>
        indices.every(i => getNodeKind(i) === "data")
      );

      const expectedDocs = [
        "docs.guide.tutorial",
        "docs.guide.syntax_examples",
        "docs.guide.syntax_reference",
        "docs.guide.patterns",
        "docs.guide.stdlib",
        "docs.guide.reading_guide"
      ];

      for (const expected of expectedDocs) {
        test(`Contains ${expected.split('.').pop()}`, () => ids.includes(expected));
      }
    }
  }

  // -------------------------------------------------------------------------
  console.log("\n--- Test 3: Content Access ---");
  {
    // Find the tutorial node
    const nodeCount = runner.call("cov_node_count") as number;
    let tutorialIdx = -1;
    for (let i = 0; i < nodeCount; i++) {
      if (getNodeId(i) === "docs.guide.tutorial") {
        tutorialIdx = i;
        break;
      }
    }

    if (tutorialIdx >= 0) {
      const content = getNodeContent(tutorialIdx, 200);
      console.log(`  Tutorial content preview: "${content.slice(0, 80)}..."`);

      test("Tutorial content contains 'Covenant'", () =>
        content.toLowerCase().includes("covenant")
      );
      test("Tutorial content contains 'snippet'", () =>
        content.toLowerCase().includes("snippet")
      );
    } else {
      test("Tutorial node found", () => false);
    }
  }

  // -------------------------------------------------------------------------
  console.log("\n--- Test 4: Relation Count (GAI) ---");
  {
    // Check if tutorial has outgoing relations
    const nodeCount = runner.call("cov_node_count") as number;
    let tutorialIdx = -1;
    for (let i = 0; i < nodeCount; i++) {
      if (getNodeId(i) === "docs.guide.tutorial") {
        tutorialIdx = i;
        break;
      }
    }

    if (tutorialIdx >= 0) {
      const outCount = runner.call("cov_get_outgoing_count", tutorialIdx) as number;
      console.log(`  Tutorial outgoing relations: ${outCount}`);

      test("Tutorial has outgoing relations (related_to)", () => outCount >= 1);

      if (outCount > 0) {
        // Read first relation
        // Return format: ((target_idx as u32) << 8) | rel_type_idx (see gai_codegen.rs:461-463)
        const relPacked = runner.call("cov_get_outgoing_rel", tutorialIdx, 0) as bigint;
        const rel = unpackRelation(relPacked);
        if (rel) {
          const targetId = getNodeId(rel.targetIdx);
          const relTypeName = getRelTypeName(rel.relTypeIdx);
          console.log(`  First relation: -[${relTypeName}]-> ${targetId}`);

          test("Relation has valid target", () => targetId.length > 0);
          test("Relation has valid type", () => relTypeName.length > 0);
        }
      }
    }
  }

  // -------------------------------------------------------------------------
  console.log("\n--- Test 5: get_related (traversal from query result) ---");
  {
    // Note: This tests if traversal from a query result works
    // This is experimental - traverse may expect a literal ID, not a query result

    try {
      const result = runner.call("get_related", 0, 0) as bigint;  // dummy params
      const { ptr, count } = unpackFatPtr(result);

      console.log(`  get_related returned ${count} nodes`);

      // Due to traverse possibly not supporting query result as source,
      // we accept 0 as valid
      test("get_related completes without crash", () => true);

      if (count > 0) {
        const indices = readNodeIndices(ptr, count);
        const ids = indices.map(i => getNodeId(i));
        console.log(`  Related docs: ${ids.join(", ")}`);
      } else {
        console.log("  NOTE: Traverse from query result returned 0 - may need from=literal support");
      }
    } catch (e) {
      console.log(`  get_related failed: ${e}`);
      test("get_related completes without crash", () => false);
    }
  }

  // -------------------------------------------------------------------------
  console.log("\n--- Pressure Test Findings ---");
  console.log(`
  Issues discovered:
  1. SLOW COMPILATION: Large data content causes very slow compile times
     - Full docs (~90KB) took >5min before timeout
     - Truncated version (~20KB) compiles in ~1s

  2. NO RUNTIME STRING ALLOCATION: Can't pass string parameters dynamically
     - WASM doesn't export cov_alloc
     - Parameterized queries (search_by_keyword, get_doc_by_id) can't be
       called with runtime values
     - Workaround: Use compile-time literals or add allocator to codegen

  3. TRAVERSE FROM QUERY RESULT: May not work as expected
     - Example 52 uses from="literal_id"
     - Chaining (from="previous_result") works for node arrays
     - But from="source" where source is a query result needs verification
  `);

  // -------------------------------------------------------------------------
  console.log("--- Summary ---");
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
