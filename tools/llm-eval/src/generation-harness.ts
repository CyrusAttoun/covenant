/**
 * Generation Harness for Covenant LLM Code Generation.
 *
 * Orchestrates the full generation pipeline:
 * 1. Load spec + examples
 * 2. Generate code via LLM API (Vercel AI SDK)
 * 3. Validate with compiler
 * 4. Collect metrics
 * 5. Self-correct if needed
 */

import { generateText } from "ai";
import { anthropic } from "@ai-sdk/anthropic";
import { openai } from "@ai-sdk/openai";
import type {
  GenerationAttempt,
  GenerationMetrics,
  GenerationTask,
  ModelConfig,
  ValidationResult,
} from "./types.ts";
import { ExampleSelector } from "./example-selector.ts";
import { CompilerValidator } from "./compiler-validator.ts";
import { extractCodeFromMarkdown } from "./utils/markdown-extractor.ts";
import { calculateCost } from "./utils/cost-calculator.ts";

const MOCK_SNIPPET = `snippet id="test.mock" kind="fn"
signature
  fn name="mock"
    returns type="Unit"
  end
end
body
  step id="s1" kind="return"
    lit=none
    as="_"
  end
end
end`;

/** Create a Vercel AI SDK model instance from config. */
function createModel(config: ModelConfig) {
  switch (config.provider) {
    case "anthropic":
      return anthropic(config.modelId);
    case "openai":
      return openai(config.modelId);
    case "mock":
      return null;
  }
}

/**
 * Orchestrates LLM code generation and validation.
 */
export class GenerationHarness {
  private readonly model: ReturnType<typeof createModel>;
  private readonly validator: CompilerValidator;
  private readonly exampleSelector: ExampleSelector;
  private readonly specContent: string;
  private readonly maxCorrectionRounds: number;
  private readonly config: ModelConfig;

  constructor(
    config: ModelConfig,
    validator: CompilerValidator,
    exampleSelector: ExampleSelector,
    specContent: string,
    maxCorrectionRounds = 2,
  ) {
    this.config = config;
    this.model = createModel(config);
    this.validator = validator;
    this.exampleSelector = exampleSelector;
    this.specContent = specContent;
    this.maxCorrectionRounds = maxCorrectionRounds;
  }

  /** Generate code for a task with automatic validation and correction. */
  async generate(task: GenerationTask, verbose = false): Promise<GenerationMetrics> {
    const startTime = performance.now();
    const attempts: GenerationAttempt[] = [];

    // Select examples
    const selected = this.exampleSelector.select(task.taskType, 1500);
    const examplesText = await this.exampleSelector.loadExamples(selected);

    if (verbose) {
      console.log(`Task: ${task.id}`);
      console.log(`Selected ${selected.length} examples`);
    }

    // Attempt 1: Initial generation
    const attempt1 = await this.generateAttempt(task, examplesText, 1, verbose);
    attempts.push(attempt1);

    const firstPassSuccess = attempt1.validationResult?.success === true;
    if (verbose) {
      console.log(`Attempt 1: ${firstPassSuccess ? "Success" : "Failed"}`);
    }

    // Self-correction rounds
    let currentCode = attempt1.generatedCode;
    let currentValidation = attempt1.validationResult;
    let finalSuccess = firstPassSuccess;

    for (let round = 2; round <= this.maxCorrectionRounds + 1; round++) {
      if (finalSuccess) break;
      if (!currentValidation || currentValidation.success) break;

      if (verbose) {
        console.log(`\nAttempt ${round}: Correcting errors...`);
        for (const err of currentValidation.errors.slice(0, 3)) {
          console.log(`  [${err.code}] ${err.message}`);
        }
      }

      const attemptN = await this.generateCorrection(
        task, examplesText, round, currentCode, currentValidation, verbose,
      );
      attempts.push(attemptN);

      currentCode = attemptN.generatedCode;
      currentValidation = attemptN.validationResult;
      finalSuccess = currentValidation?.success === true;

      if (verbose) {
        console.log(`Attempt ${round}: ${finalSuccess ? "Success" : "Failed"}`);
      }
    }

    // Calculate metrics
    const totalDurationMs = performance.now() - startTime;
    const totalPromptTokens = attempts.reduce((s, a) => s + a.promptTokens, 0);
    const totalCompletionTokens = attempts.reduce((s, a) => s + a.completionTokens, 0);
    const totalCostUsd = calculateCost(this.config.modelId, totalPromptTokens, totalCompletionTokens);

    const errorCodes = !finalSuccess && currentValidation
      ? currentValidation.errors.map((e) => e.code)
      : [];

    return {
      taskId: task.id,
      taskType: task.taskType,
      timestamp: new Date().toISOString(),
      attempts,
      totalAttempts: attempts.length,
      firstPassSuccess,
      finalSuccess,
      totalPromptTokens,
      totalCompletionTokens,
      totalDurationMs,
      totalCostUsd,
      errorCodes,
    };
  }

  /** Generate a single attempt. */
  private async generateAttempt(
    task: GenerationTask,
    examples: string,
    attemptNumber: number,
    verbose: boolean,
  ): Promise<GenerationAttempt> {
    const prompt = this.buildInitialPrompt(task, examples);
    const start = performance.now();
    const { code, promptTokens, completionTokens } = await this.callLlm(prompt, verbose);
    const durationMs = performance.now() - start;
    const validationResult = await this.validator.validate(code);

    return {
      attemptNumber,
      promptTokens,
      completionTokens,
      generatedCode: code,
      durationMs,
      validationResult,
    };
  }

  /** Generate a correction attempt. */
  private async generateCorrection(
    task: GenerationTask,
    _examples: string,
    attemptNumber: number,
    previousCode: string,
    previousValidation: ValidationResult,
    verbose: boolean,
  ): Promise<GenerationAttempt> {
    const prompt = this.buildCorrectionPrompt(task, previousCode, previousValidation);
    const start = performance.now();
    const { code, promptTokens, completionTokens } = await this.callLlm(prompt, verbose);
    const durationMs = performance.now() - start;
    const validationResult = await this.validator.validate(code);

    return {
      attemptNumber,
      promptTokens,
      completionTokens,
      generatedCode: code,
      durationMs,
      validationResult,
    };
  }

  /** Build prompt for initial generation. */
  private buildInitialPrompt(task: GenerationTask, examples: string): string {
    const paramsText = task.parameters
      .map((p) => `  - ${p.name}: ${p.type}${p.description ? ` (${p.description})` : ""}`)
      .join("\n");

    const reqsText = task.requirements
      .map((r) => `  - [${r.priority}] ${r.text}`)
      .join("\n");

    const effectsText = task.expectedEffects?.join(", ") || "none";
    const contextLine = task.context ? `\nAdditional Context: ${task.context}` : "";

    return `You are a Covenant code generator. Generate valid Covenant code following the specification exactly.

${this.specContent}

${examples}

Generate a Covenant function with the following specification:

Module: ${task.module}
Function: ${task.functionName}
Description: ${task.description}

Parameters:
${paramsText}

Returns: ${task.returnType}

Expected Effects: ${effectsText}

Requirements:
${reqsText}
${contextLine}

Generate complete snippet with:
1. effects section (if needed)
2. signature section
3. body section with step-by-step implementation
4. At least one test

Output ONLY the Covenant code, no explanation.`;
  }

  /** Build prompt for error correction. */
  private buildCorrectionPrompt(
    task: GenerationTask,
    previousCode: string,
    validation: ValidationResult,
  ): string {
    const errorsText = validation.errors
      .slice(0, 5)
      .map((e) => `[${e.code}] ${e.message}${e.line ? ` at line ${e.line}` : ""}`)
      .join("\n");

    // Include spec for context on correction too
    return `The following Covenant code has compilation errors. Fix them.

${this.specContent}

Task: ${task.description} (module: ${task.module}, function: ${task.functionName})

ORIGINAL CODE:
\`\`\`
${previousCode}
\`\`\`

COMPILER ERRORS:
${errorsText}

Generate corrected Covenant code that fixes all errors.
Preserve all functionality while fixing:
- Effect transitivity violations
- Pattern match exhaustiveness
- Canonical ordering issues
- SSA form violations
- Type mismatches

Output ONLY the corrected Covenant code, no explanation.`;
  }

  /** Call the LLM to generate code. */
  private async callLlm(
    prompt: string,
    _verbose: boolean,
  ): Promise<{ code: string; promptTokens: number; completionTokens: number }> {
    if (this.config.provider === "mock") {
      return { code: MOCK_SNIPPET, promptTokens: 100, completionTokens: 50 };
    }

    const { text, usage } = await generateText({
      model: this.model!,
      prompt,
      maxOutputTokens: this.config.maxOutputTokens ?? 4000,
    });

    const code = extractCodeFromMarkdown(text);
    return {
      code,
      promptTokens: usage.inputTokens ?? 0,
      completionTokens: usage.outputTokens ?? 0,
    };
  }
}

/** Run a suite of generation tasks. */
export async function runTestSuite(
  tasks: readonly GenerationTask[],
  harness: GenerationHarness,
  outputFile?: string,
  verbose = false,
): Promise<GenerationMetrics[]> {
  const results: GenerationMetrics[] = [];

  for (let i = 0; i < tasks.length; i++) {
    const task = tasks[i]!;
    if (verbose) {
      console.log(`\n${"=".repeat(80)}`);
      console.log(`Task ${i + 1}/${tasks.length}: ${task.id}`);
      console.log("=".repeat(80));
    }

    try {
      const metrics = await harness.generate(task, verbose);
      results.push(metrics);

      if (verbose) {
        console.log(`\nFinal: ${metrics.finalSuccess ? "Success" : "Failed"}`);
        console.log(`  Attempts: ${metrics.totalAttempts}`);
        console.log(`  Cost: $${metrics.totalCostUsd.toFixed(4)}`);
        console.log(`  Time: ${metrics.totalDurationMs.toFixed(0)}ms`);
      }

      // Save incrementally
      if (outputFile) {
        await Deno.writeTextFile(outputFile, JSON.stringify(metrics) + "\n", { append: true });
      }
    } catch (err) {
      console.error(`ERROR in task ${task.id}: ${err}`);
    }
  }

  return results;
}

/** Print summary statistics. */
export function printSummary(results: readonly GenerationMetrics[]): void {
  const total = results.length;
  if (total === 0) return;

  const firstPass = results.filter((r) => r.firstPassSuccess).length;
  const final = results.filter((r) => r.finalSuccess).length;
  const avgAttempts = results.reduce((s, r) => s + r.totalAttempts, 0) / total;
  const avgCost = results.reduce((s, r) => s + r.totalCostUsd, 0) / total;
  const avgTime = results.reduce((s, r) => s + r.totalDurationMs, 0) / total;

  console.log(`\n${"=".repeat(80)}`);
  console.log("SUMMARY");
  console.log("=".repeat(80));
  console.log(`Total tasks: ${total}`);
  console.log(`First-pass success: ${firstPass}/${total} (${(firstPass / total * 100).toFixed(1)}%)`);
  console.log(`Final success: ${final}/${total} (${(final / total * 100).toFixed(1)}%)`);
  console.log(`Average attempts: ${avgAttempts.toFixed(1)}`);
  console.log(`Average cost: $${avgCost.toFixed(4)}`);
  console.log(`Average time: ${avgTime.toFixed(0)}ms`);
  console.log();

  // Error analysis
  const errorCounts = new Map<string, number>();
  for (const r of results) {
    for (const code of r.errorCodes) {
      errorCounts.set(code, (errorCounts.get(code) ?? 0) + 1);
    }
  }

  if (errorCounts.size > 0) {
    console.log("Most common errors:");
    const sorted = [...errorCounts.entries()].sort((a, b) => b[1] - a[1]);
    for (const [code, count] of sorted.slice(0, 10)) {
      console.log(`  ${code}: ${count}`);
    }
  }
}
