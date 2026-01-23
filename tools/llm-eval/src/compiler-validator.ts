/**
 * Compiler Integration Validator for Covenant LLM Generation.
 *
 * Validates generated Covenant code using the WASM-based compiler modules
 * from host/src/loader.ts. Falls back to covenant-cli subprocess if WASM
 * modules are not available.
 */

import { CovenantHost } from "../../host/src/loader.ts";
import type {
  CompilationPhase,
  CompilerError,
  ErrorSeverity,
  ValidationResult,
} from "./types.ts";

/** Parse an error code prefix to determine which compilation phase failed. */
function inferPhaseFromErrors(errors: readonly CompilerError[]): CompilationPhase {
  for (const err of errors) {
    if (err.code.startsWith("E-PARSE")) return "parser";
    if (err.code.startsWith("E-TYPE")) return "type_check";
    if (err.code.startsWith("E-EFFECT")) return "effect_check";
    if (err.code.startsWith("E-CODEGEN")) return "codegen";
  }
  return "unknown";
}

/** Parse a structured error string into a CompilerError object. */
function parseErrorString(raw: string): CompilerError {
  // Format: "ERROR [E-PARSE-001] at line 10: Unexpected token"
  const codeMatch = raw.match(/\[([EW]-[A-Z]+-\d+)\]/);
  const code = codeMatch?.[1] ?? "E-UNKNOWN";

  const lineMatch = raw.match(/line (\d+)/);
  const line = lineMatch ? parseInt(lineMatch[1]!, 10) : undefined;

  let severity: ErrorSeverity = "error";
  if (raw.startsWith("WARNING") || raw.startsWith("warning") || code.startsWith("W-")) {
    severity = "warning";
  }

  let message = raw;
  if (codeMatch) {
    message = raw.slice(codeMatch.index! + codeMatch[0].length).trim();
    if (message.startsWith("at ")) {
      const colonIdx = message.indexOf(":");
      message = colonIdx >= 0 ? message.slice(colonIdx + 1).trim() : message;
    }
    if (message.startsWith(":")) {
      message = message.slice(1).trim();
    }
  }

  return { code, severity, message, line };
}

/**
 * Parse compiler text output (stderr+stdout) into structured errors.
 * Handles both line-by-line text and JSON formats.
 */
function parseCompilerOutput(
  stdout: string,
  stderr: string,
  returnCode: number,
): { errors: CompilerError[]; warnings: CompilerError[] } {
  const output = `${stderr}\n${stdout}`.trim();
  const errors: CompilerError[] = [];
  const warnings: CompilerError[] = [];

  // Try JSON first
  if (output.startsWith("{") || output.startsWith("[")) {
    try {
      const data = JSON.parse(output);
      const items = Array.isArray(data) ? data : [data];
      for (const item of items) {
        const severity: ErrorSeverity = item.severity ?? "error";
        const err: CompilerError = {
          code: item.code ?? "E-UNKNOWN",
          severity,
          message: item.message ?? "",
          location: item.location,
          line: item.line,
          column: item.column,
          suggestion: item.suggestion,
          autoFixConfidence: item.auto_fix_confidence,
        };
        if (severity === "error") errors.push(err);
        else warnings.push(err);
      }
      return { errors, warnings };
    } catch {
      // Fall through to text parsing
    }
  }

  // Text parsing
  for (const line of output.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;

    if (trimmed.startsWith("ERROR") || trimmed.startsWith("error")) {
      errors.push(parseErrorString(trimmed));
    } else if (trimmed.startsWith("WARNING") || trimmed.startsWith("warning")) {
      warnings.push(parseErrorString(trimmed));
    }
  }

  // Generic error if return code is non-zero but no structured errors found
  if (errors.length === 0 && returnCode !== 0) {
    errors.push({
      code: "E-UNKNOWN",
      severity: "error",
      message: output || "Compilation failed",
    });
  }

  return { errors, warnings };
}

/**
 * Validates Covenant source code.
 *
 * Primary path: in-process WASM validation via CovenantHost.
 * Fallback: shells out to covenant-cli if WASM modules are unavailable.
 */
export class CompilerValidator {
  private host: CovenantHost | null = null;
  private initAttempted = false;
  private useWasm = false;
  private readonly wasmDir: string;

  constructor(wasmDir?: string) {
    this.wasmDir = wasmDir ?? new URL("../../wasm", import.meta.url).pathname;
  }

  /** Attempt to initialize the WASM host. Idempotent. */
  private async ensureInit(): Promise<void> {
    if (this.initAttempted) return;
    this.initAttempted = true;

    try {
      this.host = new CovenantHost();
      await this.host.init(this.wasmDir);
      this.useWasm = true;
    } catch {
      console.warn(
        "WASM modules not found, falling back to covenant-cli subprocess. " +
        "Run 'cargo build --target wasm32-wasip1' to enable in-process validation.",
      );
      this.host = null;
      this.useWasm = false;
    }
  }

  /** Validate Covenant source code. */
  async validate(sourceCode: string): Promise<ValidationResult> {
    await this.ensureInit();

    if (this.useWasm && this.host?.mutation) {
      return this.validateWasm(sourceCode);
    }
    return this.validateCli(sourceCode);
  }

  /** In-process validation via WASM modules. */
  private validateWasm(sourceCode: string): ValidationResult {
    const startTime = performance.now();

    // Parse phase
    const parseResult = this.host!.mutation!.parseSnippet(sourceCode);
    if (!parseResult.success) {
      const errors = parseResult.errors.map(parseErrorString);
      const warnings = parseResult.warnings.map(parseErrorString);
      return {
        success: false,
        errors,
        warnings,
        sourceCode,
        compilationTimeMs: performance.now() - startTime,
        phaseReached: inferPhaseFromErrors(errors) || "parser",
      };
    }

    // Compile phase
    const snippetId = this.extractSnippetId(sourceCode);
    if (snippetId) {
      const compileResult = this.host!.mutation!.recompileSnippet(snippetId, sourceCode);
      if (!compileResult.success) {
        const errors = compileResult.errors.map(parseErrorString);
        return {
          success: false,
          errors,
          warnings: [],
          sourceCode,
          compilationTimeMs: performance.now() - startTime,
          phaseReached: inferPhaseFromErrors(errors),
        };
      }
    }

    return {
      success: true,
      errors: [],
      warnings: parseResult.warnings.map(parseErrorString),
      sourceCode,
      compilationTimeMs: performance.now() - startTime,
      phaseReached: "success",
    };
  }

  /** Fallback validation via covenant-cli subprocess. */
  private async validateCli(sourceCode: string): Promise<ValidationResult> {
    const startTime = performance.now();

    // Write to temp file
    const tmpFile = await Deno.makeTempFile({ suffix: ".cov" });
    try {
      await Deno.writeTextFile(tmpFile, sourceCode);

      // Try compiled binary first, then cargo run
      const projectRoot = new URL("../../", import.meta.url).pathname;
      const binaryPath = `${projectRoot}/target/debug/covenant-cli`;

      let cmd: string[];
      try {
        await Deno.stat(binaryPath);
        cmd = [binaryPath, "check", tmpFile];
      } catch {
        cmd = ["cargo", "run", "--bin", "covenant-cli", "--", "check", tmpFile];
      }

      try {
        const command = new Deno.Command(cmd[0]!, { args: cmd.slice(1), stdout: "piped", stderr: "piped" });
        const result = await command.output();

        const stdout = new TextDecoder().decode(result.stdout);
        const stderr = new TextDecoder().decode(result.stderr);
        const { errors, warnings } = parseCompilerOutput(stdout, stderr, result.code);

        const phase: CompilationPhase = errors.length === 0
          ? "success"
          : inferPhaseFromErrors(errors);

        return {
          success: errors.length === 0,
          errors,
          warnings,
          sourceCode,
          compilationTimeMs: performance.now() - startTime,
          phaseReached: phase,
        };
      } catch {
        return {
          success: false,
          errors: [{ code: "E-UNKNOWN", severity: "error", message: "Compiler not found. Run 'cargo build' first." }],
          warnings: [],
          sourceCode,
          compilationTimeMs: performance.now() - startTime,
          phaseReached: "unknown",
        };
      }
    } finally {
      try { await Deno.remove(tmpFile); } catch { /* ignore */ }
    }
  }

  /** Extract the snippet ID from source code. */
  private extractSnippetId(source: string): string | null {
    const match = source.match(/snippet\s+id="([^"]+)"/);
    return match?.[1] ?? null;
  }
}
