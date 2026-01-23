/**
 * Evaluation Runner for Covenant LLM Generation.
 *
 * CLI entry point that runs the full test suite and generates analysis reports.
 */

import { parseArgs } from "jsr:@std/cli@1/parse-args";
import type { EvaluationOptions, GenerationMetrics, ModelProvider, TaskType } from "./types.ts";
import { createTestSuite, getTestSuiteByCategory, getTestSuiteSample } from "./test-suite.ts";
import { GenerationHarness, printSummary, runTestSuite } from "./generation-harness.ts";
import { ExampleSelector } from "./example-selector.ts";
import { CompilerValidator } from "./compiler-validator.ts";

/** Load and analyze results from a JSONL file. */
async function analyzeResults(resultsPath: string): Promise<void> {
  const text = await Deno.readTextFile(resultsPath);
  const results: GenerationMetrics[] = text
    .split("\n")
    .filter((line) => line.trim())
    .map((line) => JSON.parse(line));

  const total = results.length;
  if (total === 0) {
    console.log("No results to analyze");
    return;
  }

  const firstPass = results.filter((r) => r.firstPassSuccess).length;
  const final = results.filter((r) => r.finalSuccess).length;
  const totalPrompt = results.reduce((s, r) => s + r.totalPromptTokens, 0);
  const totalCompletion = results.reduce((s, r) => s + r.totalCompletionTokens, 0);
  const totalCost = results.reduce((s, r) => s + r.totalCostUsd, 0);
  const totalTime = results.reduce((s, r) => s + r.totalDurationMs, 0);
  const avgAttempts = results.reduce((s, r) => s + r.totalAttempts, 0) / total;
  const avgCost = totalCost / total;
  const avgTime = totalTime / total;

  console.log(`\n${"=".repeat(80)}`);
  console.log("EVALUATION REPORT");
  console.log("=".repeat(80));
  console.log(`Date: ${new Date().toISOString()}`);
  console.log(`Results file: ${resultsPath}`);
  console.log();

  console.log("OVERALL METRICS");
  console.log("-".repeat(80));
  console.log(`Total tasks: ${total}`);
  console.log(`First-pass success: ${firstPass}/${total} (${(firstPass / total * 100).toFixed(1)}%)`);
  console.log(`Final success: ${final}/${total} (${(final / total * 100).toFixed(1)}%)`);
  console.log(`Improvement: ${final - firstPass} tasks (+${((final - firstPass) / total * 100).toFixed(1)}%)`);
  console.log();

  console.log("RESOURCE USAGE");
  console.log("-".repeat(80));
  console.log(`Total prompt tokens: ${totalPrompt.toLocaleString()}`);
  console.log(`Total completion tokens: ${totalCompletion.toLocaleString()}`);
  console.log(`Total tokens: ${(totalPrompt + totalCompletion).toLocaleString()}`);
  console.log(`Total cost: $${totalCost.toFixed(2)}`);
  console.log(`Total time: ${(totalTime / 1000).toFixed(1)}s`);
  console.log();

  console.log("AVERAGES PER TASK");
  console.log("-".repeat(80));
  console.log(`Average attempts: ${avgAttempts.toFixed(2)}`);
  console.log(`Average cost: $${avgCost.toFixed(4)}`);
  console.log(`Average time: ${avgTime.toFixed(0)}ms`);
  console.log(`Average tokens: ${((totalPrompt + totalCompletion) / total).toFixed(0)}`);
  console.log();

  // Success rate by task type
  console.log("SUCCESS RATE BY TASK TYPE");
  console.log("-".repeat(80));
  const byType = new Map<string, { total: number; first: number; final: number }>();
  for (const r of results) {
    const entry = byType.get(r.taskType) ?? { total: 0, first: 0, final: 0 };
    entry.total++;
    if (r.firstPassSuccess) entry.first++;
    if (r.finalSuccess) entry.final++;
    byType.set(r.taskType, entry);
  }

  for (const [taskType, stats] of [...byType.entries()].sort()) {
    const finalPct = (stats.final / stats.total * 100).toFixed(1);
    const firstPct = (stats.first / stats.total * 100).toFixed(1);
    console.log(
      `${taskType.padEnd(20)}: ${String(stats.final).padStart(3)}/${String(stats.total).padStart(3)} (${finalPct.padStart(5)}%) [first: ${firstPct.padStart(5)}%]`,
    );
  }
  console.log();

  // Error analysis
  console.log("ERROR ANALYSIS");
  console.log("-".repeat(80));
  const errorCounts = new Map<string, number>();
  for (const r of results) {
    for (const code of r.errorCodes) {
      errorCounts.set(code, (errorCounts.get(code) ?? 0) + 1);
    }
  }

  if (errorCounts.size > 0) {
    console.log("Most common errors:");
    const sorted = [...errorCounts.entries()].sort((a, b) => b[1] - a[1]);
    for (const [code, count] of sorted.slice(0, 15)) {
      console.log(`  ${code.padEnd(20)}: ${count}`);
    }
  } else {
    console.log("No errors (all tasks succeeded!)");
  }
  console.log();

  // Cost breakdown
  console.log("COST BREAKDOWN");
  console.log("-".repeat(80));
  const simple = results.filter((r) => r.totalAttempts === 1);
  const medium = results.filter((r) => r.totalAttempts === 2);
  const complex = results.filter((r) => r.totalAttempts > 2);

  if (simple.length > 0) {
    const cost = simple.reduce((s, r) => s + r.totalCostUsd, 0);
    console.log(`Simple (1 attempt):    ${String(simple.length).padStart(3)} tasks, $${cost.toFixed(2)} total, $${(cost / simple.length).toFixed(4)} avg`);
  }
  if (medium.length > 0) {
    const cost = medium.reduce((s, r) => s + r.totalCostUsd, 0);
    console.log(`Medium (2 attempts):   ${String(medium.length).padStart(3)} tasks, $${cost.toFixed(2)} total, $${(cost / medium.length).toFixed(4)} avg`);
  }
  if (complex.length > 0) {
    const cost = complex.reduce((s, r) => s + r.totalCostUsd, 0);
    console.log(`Complex (3+ attempts): ${String(complex.length).padStart(3)} tasks, $${cost.toFixed(2)} total, $${(cost / complex.length).toFixed(4)} avg`);
  }
  console.log();

  // Failure examples
  const failures = results.filter((r) => !r.finalSuccess);
  if (failures.length > 0) {
    console.log(`FAILURE EXAMPLES (showing first 5 of ${failures.length})`);
    console.log("-".repeat(80));
    for (const r of failures.slice(0, 5)) {
      console.log(`${r.taskId}:`);
      console.log(`  Type: ${r.taskType}`);
      console.log(`  Attempts: ${r.totalAttempts}`);
      console.log(`  Errors: ${r.errorCodes.join(", ") || "unknown"}`);
      console.log();
    }
  }

  // Export summary JSON
  const summaryPath = resultsPath.replace(/\.jsonl$/, ".summary.json");
  const summary = {
    date: new Date().toISOString(),
    totalTasks: total,
    firstPassSuccess: firstPass,
    firstPassRate: firstPass / total,
    finalSuccess: final,
    finalSuccessRate: final / total,
    avgAttempts,
    avgCostUsd: avgCost,
    avgTimeMs: avgTime,
    totalCostUsd: totalCost,
    totalTimeMs: totalTime,
    byType: Object.fromEntries(
      [...byType.entries()].map(([k, v]) => [k, {
        total: v.total,
        firstPassSuccess: v.first,
        finalSuccess: v.final,
        firstPassRate: v.first / v.total,
        finalSuccessRate: v.final / v.total,
      }]),
    ),
    topErrors: [...errorCounts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 10),
  };

  await Deno.writeTextFile(summaryPath, JSON.stringify(summary, null, 2));
  console.log(`Summary exported to: ${summaryPath}`);
  console.log("=".repeat(80));
}

function printUsage(): void {
  console.log(`Usage: deno task eval [options]

Options:
  --provider, -p   Model provider: anthropic | openai | mock (default: mock)
  --model, -m      Model ID (default: claude-sonnet-4-5-20250929)
  --sample, -n     Run N random tasks
  --category       Run only tasks of this category
  --output, -o     JSONL output file path
  --analyze        Analyze existing results file (no generation)
  --verbose, -v    Verbose output
  --help, -h       Show help
`);
}

async function main(): Promise<void> {
  const args = parseArgs(Deno.args, {
    string: ["provider", "model", "category", "output", "analyze", "sample"],
    boolean: ["verbose", "help"],
    alias: {
      p: "provider",
      m: "model",
      n: "sample",
      o: "output",
      v: "verbose",
      h: "help",
    },
    default: {
      provider: "mock",
      model: "claude-sonnet-4-5-20250929",
    },
  });

  if (args.help) {
    printUsage();
    Deno.exit(0);
  }

  // Analysis mode
  if (args.analyze) {
    await analyzeResults(args.analyze);
    return;
  }

  // Generation mode
  console.log("Covenant LLM Generation Evaluation");
  console.log("=".repeat(80));

  // Select tasks
  let tasks;
  if (args.sample) {
    const n = parseInt(args.sample, 10);
    console.log(`Running sample of ${n} tasks`);
    tasks = getTestSuiteSample(n);
  } else if (args.category) {
    tasks = getTestSuiteByCategory(args.category as TaskType);
    console.log(`Running ${tasks.length} tasks from category: ${args.category}`);
  } else {
    tasks = createTestSuite();
    console.log(`Running full test suite: ${tasks.length} tasks`);
  }

  // Output file
  const outputFile = args.output ?? `results_${new Date().toISOString().replace(/[:.]/g, "").slice(0, 15)}.jsonl`;

  const provider = args.provider as ModelProvider;
  console.log(`Provider: ${provider}`);
  console.log(`Model: ${args.model}`);
  console.log(`Output: ${outputFile}`);
  console.log();

  // Cost warning for real providers
  if (provider !== "mock") {
    console.log("WARNING: This will make real API calls and incur costs!");
    console.log(`Estimated cost: $${(tasks.length * 0.15).toFixed(2)} - $${(tasks.length * 0.30).toFixed(2)}`);

    const buf = new Uint8Array(10);
    await Deno.stdout.write(new TextEncoder().encode("Continue? (yes/no): "));
    const n = await Deno.stdin.read(buf);
    const response = new TextDecoder().decode(buf.subarray(0, n ?? 0)).trim().toLowerCase();
    if (response !== "yes" && response !== "y") {
      console.log("Aborted");
      return;
    }
  }

  // Load spec
  const specPath = new URL("../../llm-context/SPEC_CONDENSED.md", import.meta.url).pathname;
  let specContent: string;
  try {
    specContent = await Deno.readTextFile(specPath);
  } catch {
    console.warn(`Spec file not found at ${specPath}, using empty spec`);
    specContent = "";
  }

  // Build options
  const options: EvaluationOptions = {
    model: { provider, modelId: args.model!, maxOutputTokens: 4000 },
    maxCorrectionRounds: 2,
    maxExampleTokens: 1500,
    verbose: args.verbose ?? false,
    outputFile,
    sample: args.sample ? parseInt(args.sample, 10) : undefined,
    category: args.category as TaskType | undefined,
  };

  // Initialize components
  const validator = new CompilerValidator();
  const exampleSelector = new ExampleSelector();
  const harness = new GenerationHarness(
    options.model,
    validator,
    exampleSelector,
    specContent,
    options.maxCorrectionRounds,
  );

  // Run
  console.log("\nStarting evaluation...");
  console.log("=".repeat(80));

  const results = await runTestSuite(tasks, harness, outputFile, options.verbose);

  // Summary
  printSummary(results);

  // Detailed analysis
  console.log("\nGenerating detailed analysis...");
  await analyzeResults(outputFile);
}

if (import.meta.main) {
  main();
}
