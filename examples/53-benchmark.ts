/**
 * Performance Benchmark for Example 53
 *
 * Tests query performance with ~100 nodes (50 data + 50 function).
 * Target: Query execution < 1ms for 100 nodes.
 *
 * Scaling notes (O(n) query complexity):
 * - 100 nodes: <1ms (verified here)
 * - 1,000 nodes: ~10ms (extrapolated)
 * - 10,000 nodes: ~100ms (extrapolated, within target)
 */

import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

interface BenchmarkResult {
  name: string;
  iterations: number;
  totalMs: number;
  avgMs: number;
  passed: boolean;
  targetMs: number;
}

const benchmarks: BenchmarkResult[] = [];

function benchmark(
  name: string,
  fn: () => void,
  iterations: number,
  targetMs: number
): BenchmarkResult {
  // Warm-up
  for (let i = 0; i < 10; i++) {
    fn();
  }

  // Actual benchmark
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const totalMs = performance.now() - start;
  const avgMs = totalMs / iterations;

  const result: BenchmarkResult = {
    name,
    iterations,
    totalMs,
    avgMs,
    passed: avgMs < targetMs,
    targetMs,
  };

  benchmarks.push(result);
  return result;
}

async function main() {
  console.log("=== Performance Benchmark: 100 Node Query System ===\n");

  const runner = new CovenantQueryRunner();
  await runner.load("./examples/53-performance-benchmark.wasm");

  const memory = (runner as any).memory;

  // Helper to unpack fat pointer
  function unpackFatPtr(fatPtr: bigint): { ptr: number; count: number } {
    const ptr = Number(fatPtr >> 32n);
    const count = Number(fatPtr & 0xFFFFFFFFn);
    return { ptr, count };
  }

  // Verify node count first
  const nodeCount = runner.call("cov_node_count") as number;
  console.log(`Total nodes in WASM module: ${nodeCount}`);
  console.log("");

  // -------------------------------------------------------------------------
  // Benchmark 1: Query all nodes (no filter)
  // -------------------------------------------------------------------------
  console.log("--- Benchmark 1: Query all nodes (no filter) ---");
  const b1 = benchmark(
    "query_all",
    () => runner.call("query_all"),
    100,
    1.0 // target: <1ms
  );
  {
    const result = runner.call("query_all") as bigint;
    const { count } = unpackFatPtr(result);
    console.log(`  Result count: ${count} nodes`);
    console.log(`  Avg time: ${b1.avgMs.toFixed(3)}ms (target: <${b1.targetMs}ms)`);
    console.log(`  ${b1.passed ? "PASS" : "FAIL"}`);
  }

  // -------------------------------------------------------------------------
  // Benchmark 2: Query all data nodes (kind filter)
  // -------------------------------------------------------------------------
  console.log("\n--- Benchmark 2: Query all data nodes (kind filter) ---");
  const b2 = benchmark(
    "query_all_data",
    () => runner.call("query_all_data"),
    100,
    1.0
  );
  {
    const result = runner.call("query_all_data") as bigint;
    const { count } = unpackFatPtr(result);
    console.log(`  Result count: ${count} data nodes`);
    console.log(`  Avg time: ${b2.avgMs.toFixed(3)}ms (target: <${b2.targetMs}ms)`);
    console.log(`  ${b2.passed ? "PASS" : "FAIL"}`);
  }

  // -------------------------------------------------------------------------
  // Benchmark 3: Query all function nodes (kind filter)
  // -------------------------------------------------------------------------
  console.log("\n--- Benchmark 3: Query all function nodes (kind filter) ---");
  const b3 = benchmark(
    "query_all_fn",
    () => runner.call("query_all_fn"),
    100,
    1.0
  );
  {
    const result = runner.call("query_all_fn") as bigint;
    const { count } = unpackFatPtr(result);
    console.log(`  Result count: ${count} function nodes`);
    console.log(`  Avg time: ${b3.avgMs.toFixed(3)}ms (target: <${b3.targetMs}ms)`);
    console.log(`  ${b3.passed ? "PASS" : "FAIL"}`);
  }

  // -------------------------------------------------------------------------
  // Benchmark 4: Query by specific ID (point lookup)
  // -------------------------------------------------------------------------
  console.log("\n--- Benchmark 4: Query by specific ID (point lookup) ---");
  const b4 = benchmark(
    "query_by_id",
    () => runner.call("query_by_id"),
    100,
    0.5 // Point lookup should be faster
  );
  {
    const result = runner.call("query_by_id") as bigint;
    const { count } = unpackFatPtr(result);
    console.log(`  Result count: ${count} node(s)`);
    console.log(`  Avg time: ${b4.avgMs.toFixed(3)}ms (target: <${b4.targetMs}ms)`);
    console.log(`  ${b4.passed ? "PASS" : "FAIL"}`);
  }

  // -------------------------------------------------------------------------
  // Benchmark 5: Query with content filter (API docs)
  // -------------------------------------------------------------------------
  console.log("\n--- Benchmark 5: Query with AND + content filter ---");
  const b5 = benchmark(
    "query_api_docs",
    () => runner.call("query_api_docs"),
    100,
    1.0
  );
  {
    const result = runner.call("query_api_docs") as bigint;
    const { count } = unpackFatPtr(result);
    console.log(`  Result count: ${count} API doc nodes`);
    console.log(`  Avg time: ${b5.avgMs.toFixed(3)}ms (target: <${b5.targetMs}ms)`);
    console.log(`  ${b5.passed ? "PASS" : "FAIL"}`);
  }

  // -------------------------------------------------------------------------
  // Benchmark 6: Query with ORDER BY ascending
  // -------------------------------------------------------------------------
  console.log("\n--- Benchmark 6: Query with ORDER BY ascending ---");
  const b6 = benchmark(
    "query_sorted_asc",
    () => runner.call("query_sorted_asc"),
    100,
    2.0 // Sorting adds overhead
  );
  {
    const result = runner.call("query_sorted_asc") as bigint;
    const { count } = unpackFatPtr(result);
    console.log(`  Result count: ${count} nodes (sorted)`);
    console.log(`  Avg time: ${b6.avgMs.toFixed(3)}ms (target: <${b6.targetMs}ms)`);
    console.log(`  ${b6.passed ? "PASS" : "FAIL"}`);
  }

  // -------------------------------------------------------------------------
  // Benchmark 7: Query with ORDER BY descending
  // -------------------------------------------------------------------------
  console.log("\n--- Benchmark 7: Query with ORDER BY descending ---");
  const b7 = benchmark(
    "query_sorted_desc",
    () => runner.call("query_sorted_desc"),
    100,
    2.0
  );
  {
    const result = runner.call("query_sorted_desc") as bigint;
    const { count } = unpackFatPtr(result);
    console.log(`  Result count: ${count} nodes (sorted)`);
    console.log(`  Avg time: ${b7.avgMs.toFixed(3)}ms (target: <${b7.targetMs}ms)`);
    console.log(`  ${b7.passed ? "PASS" : "FAIL"}`);
  }

  // -------------------------------------------------------------------------
  // Benchmark 8: Query with LIMIT
  // -------------------------------------------------------------------------
  console.log("\n--- Benchmark 8: Query with LIMIT ---");
  const b8 = benchmark(
    "query_limited",
    () => runner.call("query_limited"),
    100,
    1.0
  );
  {
    const result = runner.call("query_limited") as bigint;
    const { count } = unpackFatPtr(result);
    console.log(`  Result count: ${count} nodes (limited to 10)`);
    console.log(`  Avg time: ${b8.avgMs.toFixed(3)}ms (target: <${b8.targetMs}ms)`);
    console.log(`  ${b8.passed ? "PASS" : "FAIL"}`);
  }

  // -------------------------------------------------------------------------
  // Summary
  // -------------------------------------------------------------------------
  console.log("\n=== Benchmark Summary ===\n");

  console.log("| Benchmark | Avg (ms) | Target | Status |");
  console.log("|-----------|----------|--------|--------|");
  for (const b of benchmarks) {
    const status = b.passed ? "PASS" : "FAIL";
    console.log(
      `| ${b.name.padEnd(18)} | ${b.avgMs.toFixed(3).padStart(8)} | <${b.targetMs.toFixed(1).padStart(4)}ms | ${status.padEnd(6)} |`
    );
  }

  const passed = benchmarks.filter((b) => b.passed).length;
  const total = benchmarks.length;

  console.log(`\n${passed}/${total} benchmarks passed target times`);

  if (passed === total) {
    console.log("\nPerformance targets met for 100 node queries.");
    console.log("\nScaling projection (O(n) complexity):");
    console.log("  - 100 nodes:   <1ms   (verified)");
    console.log("  - 1,000 nodes: ~10ms  (extrapolated)");
    console.log("  - 10,000 nodes: ~100ms (extrapolated, within target)");
  } else {
    console.log("\nSome benchmarks exceeded target times.");
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
