import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

async function analyze(path: string, label: string) {
  const runner = new CovenantQueryRunner();
  await runner.load(path);
  const memory = (runner as any).memory;
  const memBytes = new Uint8Array(memory.buffer);
  const memSize = memBytes.length;
  
  console.log("\n=== " + label + " ===");
  console.log("Memory size: " + memSize);
  
  // Find NULL-TERMINATED "data" (100,97,116,97,0)
  console.log("NULL-terminated 'data' (100,97,116,97,0):");
  let nullTermCount = 0;
  for (let i = 0; i < memSize - 5; i++) {
    if (memBytes[i] === 100 && memBytes[i+1] === 97 && memBytes[i+2] === 116 && memBytes[i+3] === 97 && memBytes[i+4] === 0) {
      console.log("  offset " + i + " [NULL TERMINATED]");
      nullTermCount++;
    }
  }
  if (nullTermCount === 0) console.log("  NONE FOUND!");
  
  // Find all "data" occurrences
  console.log("All 'data' occurrences:");
  for (let i = 0; i < Math.min(1000, memSize - 4); i++) {
    if (memBytes[i] === 100 && memBytes[i+1] === 97 && memBytes[i+2] === 116 && memBytes[i+3] === 97) {
      const next = memBytes[i+4];
      console.log("  offset " + i + ": next byte = " + next + " ('" + String.fromCharCode(next) + "')");
    }
  }
  
  function unpackFatPtr(fatPtr: bigint) {
    return { ptr: Number(fatPtr >> 32n), count: Number(fatPtr & 0xFFFFFFFFn) };
  }
  
  const result = runner.call("find_docs") as bigint;
  const { ptr, count } = unpackFatPtr(result);
  console.log("Query result: ptr=" + ptr + ", count=" + count);
}

await analyze("./examples/61-minimal-query.wasm", "WITH string function (WORKS)");
await analyze("./examples/62-no-string.wasm", "WITHOUT string function (BROKEN)");
