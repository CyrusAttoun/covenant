"""
Test Generation Harness for Covenant LLM Code Generation

Orchestrates the full generation pipeline:
1. Load spec + examples
2. Generate code via LLM API
3. Validate with compiler
4. Collect metrics
5. Self-correct if needed
"""

import os
import json
import time
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass, asdict, field
from datetime import datetime
from enum import Enum

from example_selector import ExampleSelector, TaskType, select_examples_for_task
from compiler_validator import CompilerValidator, ValidationResult


@dataclass
class GenerationTask:
    """Specification for a code generation task"""
    id: str
    task_type: TaskType
    description: str
    module: str
    function_name: str
    parameters: List[Dict[str, str]]
    return_type: str
    requirements: List[Dict[str, str]]
    expected_effects: List[str] = field(default_factory=list)
    context: Optional[str] = None


@dataclass
class GenerationAttempt:
    """Single generation attempt"""
    attempt_number: int
    prompt_tokens: int
    completion_tokens: int
    generated_code: str
    duration_ms: float
    validation_result: Optional[ValidationResult] = None


@dataclass
class GenerationMetrics:
    """Metrics for a single generation task"""
    task_id: str
    task_type: str
    timestamp: str

    # Attempts
    attempts: List[GenerationAttempt]
    total_attempts: int

    # Success
    first_pass_success: bool
    final_success: bool

    # Tokens
    total_prompt_tokens: int
    total_completion_tokens: int

    # Time
    total_duration_ms: float

    # Costs (assuming standard API pricing)
    total_cost_usd: float

    # Errors
    error_codes: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict:
        return asdict(self)


class ModelProvider(Enum):
    """LLM API providers"""
    ANTHROPIC = "anthropic"
    OPENAI = "openai"
    MOCK = "mock"  # For testing without API calls


class GenerationHarness:
    """Orchestrates LLM code generation and validation"""

    def __init__(self,
                 model_provider: ModelProvider = ModelProvider.ANTHROPIC,
                 model_name: str = "claude-sonnet-4-5-20250929",
                 spec_path: Optional[Path] = None,
                 max_correction_rounds: int = 2):
        """
        Initialize harness.

        Args:
            model_provider: Which LLM API to use
            model_name: Specific model identifier
            spec_path: Path to condensed spec file
            max_correction_rounds: Max self-correction attempts
        """
        self.model_provider = model_provider
        self.model_name = model_name
        self.max_correction_rounds = max_correction_rounds

        # Load spec
        if spec_path is None:
            spec_path = Path(__file__).parent / "SPEC_CONDENSED.md"
        self.spec = spec_path.read_text(encoding='utf-8')

        # Initialize components
        self.example_selector = ExampleSelector()
        self.validator = CompilerValidator()

        # Initialize API client
        self.client = self._init_api_client()

    def _init_api_client(self):
        """Initialize LLM API client"""
        if self.model_provider == ModelProvider.MOCK:
            return None

        if self.model_provider == ModelProvider.ANTHROPIC:
            try:
                import anthropic
                api_key = os.environ.get("ANTHROPIC_API_KEY")
                if not api_key:
                    raise ValueError("ANTHROPIC_API_KEY not set")
                return anthropic.Anthropic(api_key=api_key)
            except ImportError:
                raise ImportError("Install anthropic: pip install anthropic")

        if self.model_provider == ModelProvider.OPENAI:
            try:
                import openai
                api_key = os.environ.get("OPENAI_API_KEY")
                if not api_key:
                    raise ValueError("OPENAI_API_KEY not set")
                return openai.OpenAI(api_key=api_key)
            except ImportError:
                raise ImportError("Install openai: pip install openai")

        raise ValueError(f"Unsupported provider: {self.model_provider}")

    def generate(self, task: GenerationTask, verbose: bool = False) -> GenerationMetrics:
        """
        Generate code for a task with automatic validation and correction.

        Args:
            task: Task specification
            verbose: Print progress

        Returns:
            GenerationMetrics with results
        """
        start_time = time.time()
        attempts = []

        # Select examples
        selected_examples = self.example_selector.select(task.task_type, max_tokens=1500)
        examples_text = self.example_selector.load_examples(selected_examples)

        if verbose:
            print(f"Task: {task.id}")
            print(f"Selected {len(selected_examples)} examples")

        # Attempt 1: Initial generation
        attempt1 = self._generate_attempt(
            task=task,
            examples=examples_text,
            attempt_number=1,
            previous_errors=None,
            verbose=verbose
        )
        attempts.append(attempt1)

        # Check if first pass succeeded
        first_pass_success = (
            attempt1.validation_result is not None and
            attempt1.validation_result.success
        )

        if verbose:
            print(f"Attempt 1: {'✓ Success' if first_pass_success else '✗ Failed'}")

        # Self-correction rounds
        current_code = attempt1.generated_code
        current_validation = attempt1.validation_result
        final_success = first_pass_success

        for round_num in range(2, self.max_correction_rounds + 2):
            if final_success:
                break

            if current_validation is None or current_validation.success:
                break

            if verbose:
                print(f"\nAttempt {round_num}: Correcting errors...")
                for err in current_validation.errors[:3]:  # Show first 3
                    print(f"  [{err.code}] {err.message}")

            # Generate correction
            attempt_n = self._generate_correction(
                task=task,
                examples=examples_text,
                attempt_number=round_num,
                previous_code=current_code,
                errors=current_validation.errors,
                verbose=verbose
            )
            attempts.append(attempt_n)

            current_code = attempt_n.generated_code
            current_validation = attempt_n.validation_result
            final_success = (
                current_validation is not None and
                current_validation.success
            )

            if verbose:
                print(f"Attempt {round_num}: {'✓ Success' if final_success else '✗ Failed'}")

        # Calculate metrics
        total_duration = (time.time() - start_time) * 1000
        total_prompt = sum(a.prompt_tokens for a in attempts)
        total_completion = sum(a.completion_tokens for a in attempts)

        # Cost calculation (Anthropic pricing: $3/M input, $15/M output for Sonnet)
        input_cost = (total_prompt / 1_000_000) * 3.0
        output_cost = (total_completion / 1_000_000) * 15.0
        total_cost = input_cost + output_cost

        # Collect error codes
        error_codes = []
        if current_validation and not current_validation.success:
            error_codes = current_validation.get_error_codes()

        return GenerationMetrics(
            task_id=task.id,
            task_type=task.task_type.value,
            timestamp=datetime.now().isoformat(),
            attempts=attempts,
            total_attempts=len(attempts),
            first_pass_success=first_pass_success,
            final_success=final_success,
            total_prompt_tokens=total_prompt,
            total_completion_tokens=total_completion,
            total_duration_ms=total_duration,
            total_cost_usd=total_cost,
            error_codes=error_codes
        )

    def _generate_attempt(self,
                         task: GenerationTask,
                         examples: str,
                         attempt_number: int,
                         previous_errors: Optional[List] = None,
                         verbose: bool = False) -> GenerationAttempt:
        """Generate a single attempt"""

        # Build prompt
        prompt = self._build_initial_prompt(task, examples)

        # Call LLM
        start = time.time()
        generated_code, prompt_tokens, completion_tokens = self._call_llm(prompt, verbose)
        duration = (time.time() - start) * 1000

        # Validate
        validation = self.validator.validate(generated_code, verbose=verbose)

        return GenerationAttempt(
            attempt_number=attempt_number,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            generated_code=generated_code,
            duration_ms=duration,
            validation_result=validation
        )

    def _generate_correction(self,
                            task: GenerationTask,
                            examples: str,
                            attempt_number: int,
                            previous_code: str,
                            errors: List,
                            verbose: bool = False) -> GenerationAttempt:
        """Generate a correction attempt"""

        # Build correction prompt
        prompt = self._build_correction_prompt(task, previous_code, errors)

        # Call LLM
        start = time.time()
        generated_code, prompt_tokens, completion_tokens = self._call_llm(prompt, verbose)
        duration = (time.time() - start) * 1000

        # Validate
        validation = self.validator.validate(generated_code, verbose=verbose)

        return GenerationAttempt(
            attempt_number=attempt_number,
            prompt_tokens=prompt_tokens,
            completion_tokens=completion_tokens,
            generated_code=generated_code,
            duration_ms=duration,
            validation_result=validation
        )

    def _build_initial_prompt(self, task: GenerationTask, examples: str) -> str:
        """Build prompt for initial generation"""
        params_text = "\n".join(
            f"  - {p['name']}: {p['type']}" + (f" ({p.get('description', '')})" if p.get('description') else "")
            for p in task.parameters
        )

        reqs_text = "\n".join(
            f"  - [{r['priority']}] {r['text']}"
            for r in task.requirements
        )

        effects_text = ", ".join(task.expected_effects) if task.expected_effects else "none"

        return f"""You are a Covenant code generator. Generate valid Covenant code following the specification exactly.

{self.spec}

{examples}

Generate a Covenant function with the following specification:

Module: {task.module}
Function: {task.function_name}
Description: {task.description}

Parameters:
{params_text}

Returns: {task.return_type}

Expected Effects: {effects_text}

Requirements:
{reqs_text}

{f"Additional Context: {task.context}" if task.context else ""}

Generate complete snippet with:
1. effects section (if needed)
2. signature section
3. body section with step-by-step implementation
4. At least one test

Output ONLY the Covenant code, no explanation.
"""

    def _build_correction_prompt(self, task: GenerationTask,
                                 previous_code: str,
                                 errors: List) -> str:
        """Build prompt for error correction"""
        errors_text = "\n".join(
            f"[{e.code}] {e.message}" + (f" at line {e.line}" if e.line else "")
            for e in errors[:5]  # Limit to first 5 errors
        )

        return f"""The following Covenant code has compilation errors. Fix them.

ORIGINAL CODE:
```
{previous_code}
```

COMPILER ERRORS:
{errors_text}

Generate corrected Covenant code that fixes all errors.
Preserve all functionality while fixing:
- Effect transitivity violations
- Pattern match exhaustiveness
- Canonical ordering issues
- SSA form violations
- Type mismatches

Output ONLY the corrected Covenant code, no explanation.
"""

    def _call_llm(self, prompt: str, verbose: bool = False) -> Tuple[str, int, int]:
        """
        Call LLM API to generate code.

        Returns:
            (generated_code, prompt_tokens, completion_tokens)
        """
        if self.model_provider == ModelProvider.MOCK:
            # Return mock code for testing
            mock_code = '''snippet id="test.mock" kind="fn"
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
end'''
            return mock_code, 100, 50

        if self.model_provider == ModelProvider.ANTHROPIC:
            response = self.client.messages.create(
                model=self.model_name,
                max_tokens=4000,
                messages=[{
                    "role": "user",
                    "content": prompt
                }]
            )

            code = response.content[0].text
            # Extract code from markdown if present
            if "```" in code:
                code = self._extract_code_from_markdown(code)

            prompt_tokens = response.usage.input_tokens
            completion_tokens = response.usage.output_tokens

            return code, prompt_tokens, completion_tokens

        raise NotImplementedError(f"Provider {self.model_provider} not implemented")

    def _extract_code_from_markdown(self, text: str) -> str:
        """Extract code from markdown code blocks"""
        import re

        # Find code blocks
        pattern = r'```(?:covenant)?\n(.*?)```'
        matches = re.findall(pattern, text, re.DOTALL)

        if matches:
            return matches[0].strip()

        return text.strip()


def run_test_suite(tasks: List[GenerationTask],
                   output_file: Optional[str] = None,
                   verbose: bool = False) -> List[GenerationMetrics]:
    """
    Run a suite of test generations.

    Args:
        tasks: List of generation tasks
        output_file: Path to save results (JSONL format)
        verbose: Print progress

    Returns:
        List of metrics for each task
    """
    harness = GenerationHarness()
    results = []

    for i, task in enumerate(tasks, 1):
        if verbose:
            print(f"\n{'='*80}")
            print(f"Task {i}/{len(tasks)}: {task.id}")
            print(f"{'='*80}")

        try:
            metrics = harness.generate(task, verbose=verbose)
            results.append(metrics)

            if verbose:
                print(f"\n✓ Final: {'Success' if metrics.final_success else 'Failed'}")
                print(f"  Attempts: {metrics.total_attempts}")
                print(f"  Cost: ${metrics.total_cost_usd:.4f}")
                print(f"  Time: {metrics.total_duration_ms:.0f}ms")

            # Save incrementally
            if output_file:
                with open(output_file, 'a', encoding='utf-8') as f:
                    f.write(json.dumps(metrics.to_dict()) + '\n')

        except Exception as e:
            print(f"ERROR in task {task.id}: {e}")
            if verbose:
                import traceback
                traceback.print_exc()

    return results


def print_summary(results: List[GenerationMetrics]):
    """Print summary statistics"""
    total = len(results)
    first_pass = sum(1 for r in results if r.first_pass_success)
    final = sum(1 for r in results if r.final_success)

    avg_attempts = sum(r.total_attempts for r in results) / total if total > 0 else 0
    avg_cost = sum(r.total_cost_usd for r in results) / total if total > 0 else 0
    avg_time = sum(r.total_duration_ms for r in results) / total if total > 0 else 0

    print("\n" + "="*80)
    print("SUMMARY")
    print("="*80)
    print(f"Total tasks: {total}")
    print(f"First-pass success: {first_pass}/{total} ({first_pass/total*100:.1f}%)")
    print(f"Final success: {final}/{total} ({final/total*100:.1f}%)")
    print(f"Average attempts: {avg_attempts:.1f}")
    print(f"Average cost: ${avg_cost:.4f}")
    print(f"Average time: {avg_time:.0f}ms")
    print()

    # Error analysis
    from collections import Counter
    all_errors = []
    for r in results:
        all_errors.extend(r.error_codes)

    if all_errors:
        print("Most common errors:")
        for code, count in Counter(all_errors).most_common(10):
            print(f"  {code}: {count}")


# Example tasks for testing
EXAMPLE_TASKS = [
    GenerationTask(
        id="test_001_pure_add",
        task_type=TaskType.PURE_FUNCTION,
        description="Add two integers",
        module="math",
        function_name="add",
        parameters=[
            {"name": "a", "type": "Int"},
            {"name": "b", "type": "Int"}
        ],
        return_type="Int",
        requirements=[
            {"priority": "high", "text": "Must return sum of a and b"}
        ]
    ),
    GenerationTask(
        id="test_002_crud_user",
        task_type=TaskType.CRUD_OPERATION,
        description="Get user by email from database",
        module="user",
        function_name="get_by_email",
        parameters=[
            {"name": "email", "type": "String"}
        ],
        return_type="union of User (optional) and DbError",
        requirements=[
            {"priority": "critical", "text": "Must query users table by email"},
            {"priority": "high", "text": "Must return none if not found"}
        ],
        expected_effects=["database"]
    ),
]


if __name__ == "__main__":
    import sys

    # Quick test with example tasks
    print("Running test generation harness...")
    print(f"Model: {ModelProvider.MOCK.value} (set ANTHROPIC_API_KEY for real testing)")

    results = run_test_suite(EXAMPLE_TASKS, verbose=True)
    print_summary(results)
