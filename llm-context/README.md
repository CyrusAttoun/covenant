# Covenant LLM Generation - Phase 1: Proof of Concept

This directory contains the Phase 1 implementation for LLM-based Covenant code generation, as specified in the feasibility analysis plan.

## Overview

**Goal**: Validate that frontier LLM models (Claude Sonnet 4.5) can reliably generate valid Covenant code with the specification in context.

**Target Metrics** (from feasibility analysis):
- First-pass success: >70%
- After self-correction: >85%
- Cost per generation: <$0.30
- Latency: <30s

## Components

### 1. Condensed Specification (`SPEC_CONDENSED.md`)

2,800-token condensed version of the Covenant language spec optimized for LLM context.

**Key features:**
- Complete core syntax coverage
- Critical rules emphasized (SSA form, effect transitivity, etc.)
- Common error patterns documented
- Generation checklist included

### 2. Prompt Templates (`PROMPT_TEMPLATES.md`)

Reusable prompt templates for 10 common generation scenarios:

1. Simple function generation
2. CRUD operation generation
3. Complex multi-step functions
4. Type definition generation
5. Migration from imperative code
6. Error recovery / self-correction
7. Test generation
8. Database schema to binding
9. Query optimization
10. Refactoring

**Usage:**
```python
from example_selector import select_examples_for_task

# Select examples for task
examples = select_examples_for_task(
    "Generate a CRUD function for users",
    max_tokens=1500
)

# Use with your favorite template
# ... build prompt with spec + examples + task
```

### 3. Example Selector (`example_selector.py`)

Smart example selection system that picks 2-3 most relevant examples based on task type.

**Features:**
- Automatic task type inference from description
- Relevance scoring with related category matching
- Token budget management
- 15 categorized examples from the codebase

**Usage:**
```python
from example_selector import ExampleSelector, TaskType

selector = ExampleSelector()

# Automatic inference
task_type = selector.get_recommended_for_task(
    "Create a function to validate email"
)

# Select examples
examples = selector.select(task_type, max_tokens=1500, max_examples=3)

# Load example content
examples_text = selector.load_examples(examples)
```

**CLI:**
```bash
python example_selector.py "Generate a CRUD function for users"
```

### 4. Compiler Validator (`compiler_validator.py`)

Python wrapper for the Covenant compiler to validate generated code.

**Features:**
- Automatic compiler detection (uses `target/debug/covenant-cli` or `cargo run`)
- Error parsing with line numbers and codes
- Phase detection (parser, type_check, effect_check, etc.)
- Auto-fix suggestion extraction
- Performance metrics

**Usage:**
```python
from compiler_validator import CompilerValidator

validator = CompilerValidator()
result = validator.validate(generated_code)

if result.success:
    print("✓ Valid code!")
else:
    for error in result.errors:
        print(f"[{error.code}] {error.message}")
```

**CLI:**
```bash
python compiler_validator.py examples/02-pure-functions.cov
```

### 5. Generation Harness (`generation_harness.py`)

End-to-end generation pipeline with automatic validation and self-correction.

**Features:**
- Multi-provider support (Anthropic, OpenAI, Mock)
- Automatic example selection
- Self-correction loop (up to 3 attempts)
- Token counting and cost tracking
- Detailed metrics collection

**Usage:**
```python
from generation_harness import GenerationHarness, GenerationTask
from example_selector import TaskType

harness = GenerationHarness(
    model_provider=ModelProvider.ANTHROPIC,
    model_name="claude-sonnet-4-5-20250929"
)

task = GenerationTask(
    id="test_001",
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
)

metrics = harness.generate(task, verbose=True)

print(f"Success: {metrics.final_success}")
print(f"Attempts: {metrics.total_attempts}")
print(f"Cost: ${metrics.total_cost_usd:.4f}")
```

### 6. Test Suite (`test_suite.py`)

Comprehensive test suite with 100+ generation tasks covering:

- **Pure functions** (15 tasks): arithmetic, strings, recursion
- **CRUD operations** (20 tasks): create, read, update, delete for 4 entities
- **Error handling** (15 tasks): parsing, validation, safe operations
- **Pattern matching** (10 tasks): enums, options, results, trees
- **Effectful functions** (15 tasks): file I/O, HTTP, system operations
- **Complex multi-step** (15 tasks): registration, payments, imports, etc.
- **Query tasks** (15 tasks): Covenant dialect and SQL dialects

**Usage:**
```python
from test_suite import create_test_suite, get_test_suite_sample

# Full suite
all_tasks = create_test_suite()  # 100+ tasks

# Random sample
sample = get_test_suite_sample(n=20)

# By category
from test_suite import get_test_suite_by_category
crud_tasks = get_test_suite_by_category(TaskType.CRUD_OPERATION)
```

### 7. Evaluation Runner (`run_evaluation.py`)

Command-line tool to run evaluations and generate analysis reports.

**Usage:**

```bash
# Run full suite (mock mode for testing)
python run_evaluation.py

# Run sample of 20 tasks
python run_evaluation.py --sample 20

# Run specific category
python run_evaluation.py --category pure_function

# Run with real API (requires ANTHROPIC_API_KEY)
python run_evaluation.py --provider anthropic --sample 10

# Analyze existing results
python run_evaluation.py --analyze results_20260110_123456.jsonl
```

**Output:**
- JSONL file with detailed metrics per task
- Summary JSON with aggregate statistics
- Console report with:
  - Overall success rates
  - Resource usage (tokens, cost, time)
  - Success rate by task type
  - Error analysis
  - Cost breakdown
  - Failure examples

## Quick Start

### 1. Install Dependencies

```bash
cd llm-context
pip install anthropic  # or: pip install openai
```

### 2. Set API Key

```bash
export ANTHROPIC_API_KEY=your_key_here
# or for OpenAI:
export OPENAI_API_KEY=your_key_here
```

### 3. Build Covenant Compiler

```bash
cd ..
cargo build
```

### 4. Run a Small Test

```bash
cd llm-context

# Test with mock provider (no API calls)
python run_evaluation.py --sample 5

# Test with real API
python run_evaluation.py --provider anthropic --sample 5 --verbose
```

### 5. Run Full Evaluation

```bash
# This will make 100+ API calls - estimated cost $15-30
python run_evaluation.py --provider anthropic --output results.jsonl
```

### 6. Analyze Results

```bash
python run_evaluation.py --analyze results.jsonl
```

## File Structure

```
llm-context/
├── README.md                    # This file
├── SPEC_CONDENSED.md           # 2.8k token specification
├── PROMPT_TEMPLATES.md         # Prompt templates
├── example_selector.py         # Example selection
├── compiler_validator.py       # Compiler integration
├── generation_harness.py       # Generation pipeline
├── test_suite.py               # 100+ test tasks
├── run_evaluation.py           # Evaluation runner
└── results/                    # Generated results (gitignored)
    ├── results_*.jsonl         # Detailed metrics
    └── results_*.summary.json  # Summary statistics
```

## Expected Results (Phase 1 Success Criteria)

Based on the feasibility analysis, Phase 1 is successful if:

✅ **First-pass success rate**: >70%
✅ **After self-correction**: >85%
✅ **Cost per generation**: <$0.30
✅ **Latency**: <30s

**Primary failure modes to monitor:**
- Effect transitivity reasoning (25-35% error rate expected)
- Pattern match exhaustiveness (20-35% error rate expected)
- Canonical ordering violations (10-15% - should auto-fix)
- Complex query construction (30-40% error rate expected)

## Next Steps

If Phase 1 succeeds:

### Phase 2: Alpha Deployment (Months 3-6)
- Build IDE integration (VS Code extension)
- Implement error recovery loop
- Add telemetry (track patterns, success rates, edits)
- Deploy to 10-20 internal users
- Collect 1k-5k real generations with feedback

### Phase 3: Data Collection (Months 6-12)
- Label collected generations
- Generate synthetic training data
- Reach 5k-10k curated examples
- Build automated evaluation harness
- Run preliminary fine-tuning experiments (7B models)

### Phase 4: Production Decision (Month 12)
- Decide: continue frontier or transition to fine-tuned?
- Criteria: monthly volume, language stability, quality needs, budget

## Cost Estimates

**Mock mode**: $0 (for testing)

**Single generation** (Anthropic Claude Sonnet 4.5):
- Input: ~4,500 tokens × $3/M = $0.0135
- Output: ~600 tokens × $15/M = $0.009
- **Total**: ~$0.023 per generation (first attempt)
- **With corrections**: ~$0.10-0.30 (matches feasibility analysis)

**Full suite** (100 tasks):
- Expected: $10-30 depending on success rates

**Sample run** (20 tasks):
- Expected: $2-6

## Troubleshooting

### Compiler not found
```bash
# Build the compiler first
cd ..
cargo build
```

### API key issues
```bash
# Check environment variable
echo $ANTHROPIC_API_KEY

# Set it if needed
export ANTHROPIC_API_KEY=your_key_here
```

### Module import errors
```bash
# Ensure you're in llm-context directory
cd llm-context

# Or add to PYTHONPATH
export PYTHONPATH="${PYTHONPATH}:$(pwd)"
```

### Example files not found
The example selector expects example files in `../examples/`. Ensure you're running from the `llm-context/` directory.

## Contributing

This is Phase 1 (Proof of Concept). If you want to:

- **Add more test tasks**: Edit `test_suite.py`
- **Add new prompt templates**: Edit `PROMPT_TEMPLATES.md`
- **Improve example selection**: Edit `example_selector.py`
- **Enhance compiler integration**: Edit `compiler_validator.py`
- **Add new LLM providers**: Edit `generation_harness.py`

## References

- Feasibility analysis: `../.claude/plans/llm-generation-feasibility-analysis.md`
- Language spec: `../DESIGN.md`, `../grammar.ebnf`
- Error codes: `../ERROR_CODES.md`
- Examples: `../examples/*.cov`

## License

Same as Covenant project.
