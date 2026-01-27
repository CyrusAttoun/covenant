/**
 * Debug script to inspect GAI function outputs
 */

import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

async function main() {
  console.log("=== GAI Debug ===\n");

  const runner = new CovenantQueryRunner();

  // Load the compiled WASM module
  await runner.load("./examples/50-embedded-query-simple.wasm");

  // Call node_count directly
  const nodeCount = runner.call("cov_node_count");
  console.log(`cov_node_count() = ${nodeCount}`);
  console.log();

  // Call get_node_id and get_node_kind for each index
  for (let i = 0; i < (nodeCount as number); i++) {
    const idFatPtr = runner.call("cov_get_node_id", i);
    console.log(`Index ${i}: cov_get_node_id(${i}) = ${idFatPtr} (0x${(idFatPtr as bigint).toString(16)})`);

    // Unpack fat pointer
    const ptr = Number((idFatPtr as bigint) >> 32n);
    const len = Number((idFatPtr as bigint) & 0xFFFFFFFFn);
    console.log(`  ID unpacked: ptr=${ptr}, len=${len}`);

    // Read memory
    if (len > 0 && len < 1000) {
      const memory = (runner as any).memory;
      const view = new Uint8Array(memory.buffer, ptr, len);
      const str = new TextDecoder().decode(view);
      console.log(`  ID: "${str}"`);
    }

    // Get kind
    const kindFatPtr = runner.call("cov_get_node_kind", i);
    const kindPtr = Number((kindFatPtr as bigint) >> 32n);
    const kindLen = Number((kindFatPtr as bigint) & 0xFFFFFFFFn);
    console.log(`  Kind unpacked: ptr=${kindPtr}, len=${kindLen}`);
    if (kindLen > 0 && kindLen < 1000) {
      const memory = (runner as any).memory;
      const view = new Uint8Array(memory.buffer, kindPtr, kindLen);
      const kindStr = new TextDecoder().decode(view);
      console.log(`  Kind: "${kindStr}"`);
    }
    console.log();
  }

  // Test find_docs query
  console.log("\n=== Testing find_docs() ===");
  const result = runner.call("find_docs");
  console.log(`find_docs() = ${result} (0x${(result as bigint).toString(16)})`);

  // Unpack result fat pointer
  const resultPtr = Number((result as bigint) >> 32n);
  const resultCount = Number((result as bigint) & 0xFFFFFFFFn);
  console.log(`Result: ptr=${resultPtr}, count=${resultCount}`);

  if (resultCount > 0) {
    console.log("Result node indices:");
    const memory = (runner as any).memory;
    const view = new DataView(memory.buffer, resultPtr, resultCount * 4);
    for (let i = 0; i < resultCount; i++) {
      const nodeIdx = view.getUint32(i * 4, true);
      console.log(`  [${i}] = ${nodeIdx}`);
    }
  } else {
    console.log("No results returned");
  }
}

if (import.meta.main) {
  await main();
}
