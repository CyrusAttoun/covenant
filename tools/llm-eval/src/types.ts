/**
 * Core types for the Covenant LLM evaluation system.
 */

/** Categories of generation tasks. */
export type TaskType =
  | "pure_function"
  | "effectful_function"
  | "crud_operation"
  | "error_handling"
  | "pattern_matching"
  | "type_definition"
  | "database_binding"
  | "query_covenant"
  | "query_sql"
  | "transaction"
  | "migration"
  | "refactoring"
  | "general";

/** Severity levels for compiler diagnostics. */
export type ErrorSeverity = "error" | "warning" | "info";

/** Compilation phase identifiers. */
export type CompilationPhase =
  | "lexer"
  | "parser"
  | "type_check"
  | "effect_check"
  | "codegen"
  | "success"
  | "unknown";

/** A single compiler diagnostic. */
export interface CompilerError {
  readonly code: string;
  readonly severity: ErrorSeverity;
  readonly message: string;
  readonly location?: string;
  readonly line?: number;
  readonly column?: number;
  readonly suggestion?: string;
  readonly autoFixConfidence?: number;
}

/** Result of validating generated Covenant code. */
export interface ValidationResult {
  readonly success: boolean;
  readonly errors: readonly CompilerError[];
  readonly warnings: readonly CompilerError[];
  readonly sourceCode: string;
  readonly compilationTimeMs?: number;
  readonly phaseReached?: CompilationPhase;
}

/** A Covenant example file with metadata for selection. */
export interface Example {
  readonly path: string;
  readonly title: string;
  readonly categories: readonly TaskType[];
  readonly approxTokens: number;
  readonly priority: number;
}

/** A parameter in a generation task. */
export interface TaskParameter {
  readonly name: string;
  readonly type: string;
  readonly description?: string;
}

/** A requirement for a generation task. */
export interface TaskRequirement {
  readonly priority: string;
  readonly text: string;
}

/** Specification for a code generation task. */
export interface GenerationTask {
  readonly id: string;
  readonly taskType: TaskType;
  readonly description: string;
  readonly module: string;
  readonly functionName: string;
  readonly parameters: readonly TaskParameter[];
  readonly returnType: string;
  readonly requirements: readonly TaskRequirement[];
  readonly expectedEffects?: readonly string[];
  readonly context?: string;
}

/** A single generation attempt (one LLM call + validation). */
export interface GenerationAttempt {
  readonly attemptNumber: number;
  readonly promptTokens: number;
  readonly completionTokens: number;
  readonly generatedCode: string;
  readonly durationMs: number;
  readonly validationResult?: ValidationResult;
}

/** Aggregate metrics for a generation task. */
export interface GenerationMetrics {
  readonly taskId: string;
  readonly taskType: string;
  readonly timestamp: string;
  readonly attempts: readonly GenerationAttempt[];
  readonly totalAttempts: number;
  readonly firstPassSuccess: boolean;
  readonly finalSuccess: boolean;
  readonly totalPromptTokens: number;
  readonly totalCompletionTokens: number;
  readonly totalDurationMs: number;
  readonly totalCostUsd: number;
  readonly errorCodes: readonly string[];
}

/** LLM provider identifiers. */
export type ModelProvider = "anthropic" | "openai" | "mock";

/** Model configuration. */
export interface ModelConfig {
  readonly provider: ModelProvider;
  readonly modelId: string;
  readonly maxOutputTokens?: number;
}

/** Options for the evaluation runner. */
export interface EvaluationOptions {
  readonly model: ModelConfig;
  readonly maxCorrectionRounds: number;
  readonly maxExampleTokens: number;
  readonly verbose: boolean;
  readonly outputFile?: string;
  readonly sample?: number;
  readonly category?: TaskType;
}
