/**
 * Covenant LLM Evaluation System
 *
 * TypeScript implementation using the Vercel AI SDK for model independence
 * and direct WASM compiler integration for validation.
 */

export type {
  CompilationPhase,
  CompilerError,
  ErrorSeverity,
  EvaluationOptions,
  Example,
  GenerationAttempt,
  GenerationMetrics,
  GenerationTask,
  ModelConfig,
  ModelProvider,
  TaskParameter,
  TaskRequirement,
  TaskType,
  ValidationResult,
} from "./types.ts";

export { ExampleSelector, inferTaskType, selectExamplesForTask } from "./example-selector.ts";
export { CompilerValidator } from "./compiler-validator.ts";
export { GenerationHarness, printSummary, runTestSuite } from "./generation-harness.ts";
export { createTestSuite, getTestSuiteByCategory, getTestSuiteSample } from "./test-suite.ts";
export { extractCodeFromMarkdown } from "./utils/markdown-extractor.ts";
export { calculateCost } from "./utils/cost-calculator.ts";
