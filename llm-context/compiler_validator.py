"""
Compiler Integration Validator for Covenant LLM Generation

Validates generated Covenant code by running it through the compiler.
Collects errors, warnings, and auto-fix suggestions.
"""

import subprocess
import json
import tempfile
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass, asdict
from enum import Enum


class ErrorSeverity(Enum):
    ERROR = "error"
    WARNING = "warning"
    INFO = "info"


@dataclass
class CompilerError:
    """Represents a compiler error or warning"""
    code: str  # E-PARSE-001, E-TYPE-005, etc.
    severity: ErrorSeverity
    message: str
    location: Optional[str] = None
    line: Optional[int] = None
    column: Optional[int] = None
    suggestion: Optional[str] = None
    auto_fix_confidence: Optional[float] = None

    def to_dict(self) -> Dict:
        return asdict(self)


@dataclass
class ValidationResult:
    """Result of validating generated code"""
    success: bool
    errors: List[CompilerError]
    warnings: List[CompilerError]
    source_code: str
    compilation_time_ms: Optional[float] = None
    phase_reached: Optional[str] = None  # "lexer", "parser", "type_check", "codegen", "success"

    def to_dict(self) -> Dict:
        return {
            "success": self.success,
            "errors": [e.to_dict() for e in self.errors],
            "warnings": [w.to_dict() for w in self.warnings],
            "source_code": self.source_code,
            "compilation_time_ms": self.compilation_time_ms,
            "phase_reached": self.phase_reached,
        }

    def has_fixable_errors(self) -> bool:
        """Check if any errors have auto-fix suggestions"""
        return any(e.auto_fix_confidence and e.auto_fix_confidence > 0.7
                   for e in self.errors)

    def get_error_codes(self) -> List[str]:
        """Get list of error codes"""
        return [e.code for e in self.errors]


class CompilerValidator:
    """Validates Covenant code using the compiler"""

    def __init__(self, compiler_path: Optional[Path] = None):
        """
        Initialize validator.

        Args:
            compiler_path: Path to covenant compiler binary
                          (defaults to target/debug/covenant-cli or cargo run)
        """
        self.compiler_path = compiler_path
        if self.compiler_path is None:
            # Try to find compiler binary
            project_root = Path(__file__).parent.parent
            debug_bin = project_root / "target" / "debug" / "covenant-cli"
            if debug_bin.exists():
                self.compiler_path = debug_bin
            else:
                # Fall back to cargo run
                self.compiler_path = None

    def validate(self, source_code: str, verbose: bool = False) -> ValidationResult:
        """
        Validate Covenant source code.

        Args:
            source_code: Covenant source code to validate
            verbose: Print detailed output

        Returns:
            ValidationResult with errors and warnings
        """
        import time

        start_time = time.time()

        # Write source to temporary file
        with tempfile.NamedTemporaryFile(mode='w', suffix='.cov', delete=False,
                                         encoding='utf-8') as f:
            f.write(source_code)
            temp_path = Path(f.name)

        try:
            # Run compiler
            result = self._run_compiler(temp_path, verbose)

            # Parse result
            errors, warnings = self._parse_compiler_output(result)

            # Determine phase reached
            phase = self._determine_phase(result, errors)

            compilation_time = (time.time() - start_time) * 1000

            return ValidationResult(
                success=len(errors) == 0,
                errors=errors,
                warnings=warnings,
                source_code=source_code,
                compilation_time_ms=compilation_time,
                phase_reached=phase
            )

        finally:
            # Clean up temp file
            temp_path.unlink(missing_ok=True)

    def _run_compiler(self, source_path: Path, verbose: bool) -> subprocess.CompletedProcess:
        """Run the compiler on a source file"""
        if self.compiler_path:
            cmd = [str(self.compiler_path), "check", str(source_path)]
        else:
            # Use cargo run
            cmd = ["cargo", "run", "--bin", "covenant-cli", "--",
                   "check", str(source_path)]

        if verbose:
            print(f"Running: {' '.join(cmd)}")

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=30,
                encoding='utf-8'
            )
            return result
        except subprocess.TimeoutExpired:
            # Return fake result with timeout error
            return subprocess.CompletedProcess(
                args=cmd,
                returncode=1,
                stdout="",
                stderr="ERROR: Compilation timeout (30s)"
            )
        except FileNotFoundError:
            # Compiler not found
            return subprocess.CompletedProcess(
                args=cmd,
                returncode=1,
                stdout="",
                stderr="ERROR: Compiler not found. Run 'cargo build' first."
            )

    def _parse_compiler_output(self, result: subprocess.CompletedProcess) -> Tuple[List[CompilerError], List[CompilerError]]:
        """
        Parse compiler output to extract errors and warnings.

        Returns:
            (errors, warnings) tuple
        """
        errors = []
        warnings = []

        output = result.stderr + "\n" + result.stdout

        # Try to parse as JSON first (if compiler supports structured output)
        if output.strip().startswith('{') or output.strip().startswith('['):
            try:
                data = json.loads(output)
                return self._parse_json_output(data)
            except json.JSONDecodeError:
                pass

        # Parse text output
        for line in output.split('\n'):
            line = line.strip()
            if not line:
                continue

            # Parse error/warning lines
            # Format: "ERROR [E-PARSE-001] at line 10: Unexpected token"
            # or: "WARNING [W-STYLE-001]: Consider using more descriptive names"

            if line.startswith("ERROR") or line.startswith("error"):
                error = self._parse_error_line(line, ErrorSeverity.ERROR)
                if error:
                    errors.append(error)

            elif line.startswith("WARNING") or line.startswith("warning"):
                warning = self._parse_error_line(line, ErrorSeverity.WARNING)
                if warning:
                    warnings.append(warning)

        # If no structured errors found but return code != 0, create generic error
        if not errors and result.returncode != 0:
            errors.append(CompilerError(
                code="E-UNKNOWN",
                severity=ErrorSeverity.ERROR,
                message=output or "Compilation failed",
                location=None
            ))

        return errors, warnings

    def _parse_error_line(self, line: str, severity: ErrorSeverity) -> Optional[CompilerError]:
        """Parse a single error/warning line"""
        import re

        # Try to extract error code
        code_match = re.search(r'\[([EW]-[A-Z]+-\d+)\]', line)
        code = code_match.group(1) if code_match else "E-UNKNOWN"

        # Try to extract line number
        line_match = re.search(r'line (\d+)', line)
        line_num = int(line_match.group(1)) if line_match else None

        # Extract message (everything after the code)
        if code_match:
            message = line[code_match.end():].strip()
            if message.startswith(':'):
                message = message[1:].strip()
        else:
            message = line

        return CompilerError(
            code=code,
            severity=severity,
            message=message,
            line=line_num
        )

    def _parse_json_output(self, data: Dict) -> Tuple[List[CompilerError], List[CompilerError]]:
        """Parse structured JSON output from compiler"""
        errors = []
        warnings = []

        if isinstance(data, dict):
            data = [data]

        for item in data:
            severity = ErrorSeverity(item.get("severity", "error"))
            error = CompilerError(
                code=item.get("code", "E-UNKNOWN"),
                severity=severity,
                message=item.get("message", ""),
                location=item.get("location"),
                line=item.get("line"),
                column=item.get("column"),
                suggestion=item.get("suggestion"),
                auto_fix_confidence=item.get("auto_fix_confidence")
            )

            if severity == ErrorSeverity.ERROR:
                errors.append(error)
            else:
                warnings.append(error)

        return errors, warnings

    def _determine_phase(self, result: subprocess.CompletedProcess,
                        errors: List[CompilerError]) -> str:
        """Determine which compilation phase was reached"""
        if not errors:
            return "success"

        # Check error codes to determine phase
        error_codes = [e.code for e in errors]

        if any(code.startswith("E-PARSE") for code in error_codes):
            return "parser"
        if any(code.startswith("E-TYPE") for code in error_codes):
            return "type_check"
        if any(code.startswith("E-EFFECT") for code in error_codes):
            return "effect_check"
        if any(code.startswith("E-CODEGEN") for code in error_codes):
            return "codegen"

        # Check output for phase indicators
        output = result.stderr + result.stdout
        if "parsing" in output.lower():
            return "parser"
        if "type checking" in output.lower():
            return "type_check"

        return "unknown"

    def validate_and_suggest_fixes(self, source_code: str) -> Tuple[ValidationResult, Optional[str]]:
        """
        Validate code and generate suggested fixes.

        Returns:
            (ValidationResult, suggested_fixed_code or None)
        """
        result = self.validate(source_code)

        if result.success or not result.has_fixable_errors():
            return result, None

        # Generate fixes (placeholder - would integrate with auto-fix system)
        fixed_code = self._apply_auto_fixes(source_code, result.errors)

        return result, fixed_code

    def _apply_auto_fixes(self, source_code: str, errors: List[CompilerError]) -> Optional[str]:
        """
        Apply auto-fixes to source code.

        This is a placeholder. Real implementation would:
        1. Parse error suggestions
        2. Apply fixes in order
        3. Re-validate
        4. Return fixed code
        """
        # For now, return None (not implemented)
        # Real implementation would use ERROR_CODES.md suggestions
        return None


def validate_file(file_path: str, verbose: bool = False) -> ValidationResult:
    """
    Convenience function to validate a Covenant file.

    Args:
        file_path: Path to .cov file
        verbose: Print detailed output

    Returns:
        ValidationResult
    """
    source = Path(file_path).read_text(encoding='utf-8')
    validator = CompilerValidator()
    return validator.validate(source, verbose=verbose)


# CLI for testing
if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        print("Usage: python compiler_validator.py <file.cov>")
        print("\nExample:")
        print("  python compiler_validator.py examples/02-pure-functions.cov")
        sys.exit(1)

    file_path = sys.argv[1]
    verbose = "-v" in sys.argv or "--verbose" in sys.argv

    print(f"Validating: {file_path}")
    print("=" * 80)

    result = validate_file(file_path, verbose=verbose)

    print(f"\nPhase reached: {result.phase_reached}")
    print(f"Compilation time: {result.compilation_time_ms:.2f}ms")
    print()

    if result.errors:
        print(f"Errors ({len(result.errors)}):")
        for err in result.errors:
            location = f" at line {err.line}" if err.line else ""
            print(f"  [{err.code}]{location}: {err.message}")
            if err.suggestion:
                print(f"    Suggestion: {err.suggestion}")
        print()

    if result.warnings:
        print(f"Warnings ({len(result.warnings)}):")
        for warn in result.warnings:
            location = f" at line {warn.line}" if warn.line else ""
            print(f"  [{warn.code}]{location}: {warn.message}")
        print()

    if result.success:
        print("✓ Validation successful!")
    else:
        print("✗ Validation failed")
        sys.exit(1)
