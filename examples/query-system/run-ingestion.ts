/**
 * Run the doc-ingestion WASM module
 *
 * This script loads and executes the compiled doc-ingestion.wasm,
 * which reads documentation files from docs/guide/ and generates
 * .cov data files in output/.
 *
 * Usage:
 *   deno run --allow-read --allow-write run-ingestion.ts
 */

import { CovenantQueryRunner } from "../../runtime/host/query-runner.ts";

async function main() {
  const runner = new CovenantQueryRunner();

  try {
    console.log("Loading doc-ingestion.wasm...");
    await runner.load("./output/doc-ingestion.wasm");

    console.log("Running main()...");
    runner.call("main");

    console.log("Ingestion complete.");
  } catch (e) {
    console.error("Error running ingestion:", e);
    Deno.exit(1);
  }
}

await main();
