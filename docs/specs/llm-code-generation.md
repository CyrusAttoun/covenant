# Specification: LLM Code Generation for Covenant

**Status**: Phase 1 Implemented
**Version**: 1.0
**Last Updated**: 2026-01-11

---

## Overview

The Covenant project shall provide an LLM-based code generation system that enables users to generate valid Covenant code from natural language descriptions. The system shall validate generated code through compiler integration and automatically correct errors.

## System Components

### 1. Specification Context

The system shall maintain a condensed specification document optimized for LLM context:

- **Token budget**: 2,500-3,000 tokens
- **Content coverage**: Core syntax, critical rules, common patterns, error checklist
- **Format**: Markdown with code examples
- **Location**: `llm-context/SPEC_CONDENSED.md`

The condensed specification shall include:
- Core philosophy and design principles
- Snippet structure templates
- Complete step types with examples
- Database bindings and SQL dialect syntax
- Common generation errors with fixes
- Generation checklist

### 2. Example Selection

The system shall provide intelligent example selection based on task type:

- **Example catalog**: Minimum 15 categorized examples
- **Selection algorithm**: Relevance scoring with related category matching
- **Token budget**: 1,000-1,500 tokens for selected examples
- **Selection count**: 2-3 examples per generation task

Supported task categories:
- Pure functions
- CRUD operations
- Error handling
- Pattern matching
- Type definitions
- Database bindings
- Queries (Covenant and SQL dialects)
- Transactions
- Effectful functions
- Complex multi-step operations
- Code migration
- Refactoring

### 3. Prompt Templates

The system shall provide reusable prompt templates for common generation scenarios:

1. Simple function generation
2. CRUD function generation
3. Complex multi-step function generation
4. Type definition generation
5. Migration from imperative languages
6. Error recovery and self-correction
7. Test generation
8. Database schema to binding
9. Query optimization
10. Refactoring

Each template shall:
- Support parameterization
- Include context size budgets
- Provide example selection guidance
- Specify expected output format

### 4. Compiler Integration

The system shall integrate with the Covenant compiler for validation:

- **Compiler detection**: Automatically locate `covenant-cli` binary or use `cargo run`
- **Error parsing**: Extract error codes, messages, line numbers, and suggestions
- **Phase detection**: Identify compilation phase (parser, type_check, effect_check, codegen)
- **Metrics collection**: Compilation time, success status, phase reached
- **Timeout handling**: 30-second maximum compilation time

Validation results shall include:
- Success/failure status
- List of errors with structured metadata
- List of warnings
- Compilation time in milliseconds
- Phase reached identifier

### 5. Generation Pipeline

The system shall provide an end-to-end generation pipeline:

**Supported LLM providers**:
- Anthropic (Claude models)
- OpenAI (GPT models)
- Mock (for testing without API calls)

**Generation process**:
1. Task specification provided by user
2. Automatic example selection based on task type
3. Prompt construction with spec + examples + task
4. LLM API call to generate code
5. Compiler validation of generated code
6. Automatic self-correction if validation fails (up to 3 attempts)
7. Metrics collection and reporting

**Self-correction loop**:
- Maximum correction attempts: 3
- Error-driven correction prompts
- Automatic retry with compiler feedback
- Success determination: compiler validation passes

**Metrics tracked per task**:
- Attempt count
- First-pass success (boolean)
- Final success (boolean)
- Total prompt tokens
- Total completion tokens
- Total cost (USD)
- Total duration (milliseconds)
- Error codes encountered

### 6. Test Suite

The system shall include a comprehensive test suite:

- **Total tasks**: Minimum 100 tasks
- **Coverage**: All major language patterns and complexity levels
- **Categories**: Pure functions, CRUD, error handling, pattern matching, effects, complex operations, queries

Task distribution:
- Pure functions: 15+ tasks (arithmetic, strings, recursion)
- CRUD operations: 20+ tasks (create, read, update, delete across multiple entities)
- Error handling: 15+ tasks (parsing, validation, safe operations)
- Pattern matching: 10+ tasks (enums, options, results, trees)
- Effectful functions: 15+ tasks (I/O, HTTP, system operations)
- Complex multi-step: 15+ tasks (registration, payments, imports, analytics)
- Query tasks: 15+ tasks (Covenant and SQL dialects)

### 7. Evaluation and Analysis

The system shall provide evaluation and analysis capabilities:

**Evaluation modes**:
- Full suite execution
- Sample execution (random subset)
- Category-specific execution
- Results analysis

**Output formats**:
- JSONL results file (detailed metrics per task)
- JSON summary file (aggregate statistics)
- Console report (human-readable summary)

**Analysis reports shall include**:
- Overall success rates (first-pass and final)
- Resource usage (tokens, cost, time)
- Average metrics per task
- Success rate by task type
- Error analysis (frequency distribution)
- Cost breakdown by complexity
- Failure examples

## Performance Requirements

### Phase 1 Success Criteria

The system shall meet the following performance targets for Phase 1:

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| First-pass success rate | >70% | Percentage of tasks succeeding without correction |
| Final success rate | >85% | Percentage of tasks succeeding after self-correction |
| Cost per generation | <$0.30 USD | Average cost including correction attempts |
| Latency | <30 seconds | Average total time per task |

### Expected Performance Ranges

**Syntax correctness**: 85-95%
- Primary error mode: Canonical ordering violations

**Semantic correctness**: 70-85%
- Challenging areas: Effect transitivity, pattern exhaustiveness

**Self-correction effectiveness**: 90-95%
- With compiler errors and auto-fix suggestions

### Known Failure Modes

The system shall handle the following expected failure modes:

| Failure Mode | Expected Error Rate | Mitigation Strategy |
|--------------|---------------------|---------------------|
| Effect transitivity reasoning | 25-35% | Compiler provides full call chain in errors |
| Pattern match exhaustiveness | 20-35% | Compiler lists missing variants explicitly |
| Canonical ordering violations | 10-15% | Auto-fix with confidence 1.0 (deterministic) |
| Complex query construction | 30-40% | Examples in context + query cost analysis |
| ID naming consistency | 5-10% | Template patterns, easy self-correction |

## Cost Model

### Frontier Model Approach (Phase 1)

**Per-generation costs** (Anthropic Claude Sonnet 4.5):

| Scenario | Cost Range | Use Case |
|----------|-----------|----------|
| Simple function | $0.08-0.09 | Basic CRUD, pure functions |
| Medium function | $0.11-0.12 | Multi-effect, pattern matching |
| Complex function | $0.16-0.17 | Deep call chains, AST queries |
| With error correction | $0.25-0.30 | Expected average with 2 correction rounds |

**Economic viability**: Cost <$0.30/generation is acceptable for developer tooling, migration assistance, and rapid prototyping.

### Fine-Tuned Model Approach (Future Phase)

**Upfront investment**: $150k-$300k
- Data generation and curation: $130k-$250k
- Model fine-tuning: $10k-$20k
- Evaluation infrastructure: $10k-$20k

**Per-generation cost**: $0.00075

**Break-even point**: 2,000-3,000 generations/day (60k-90k/month)

## Deployment Patterns

The system shall support the following deployment patterns:

### 1. Interactive IDE Assistant
User describes function → LLM generates → Compiler validates → LLM corrects → User approves

### 2. Migration Tool
Translate Python/JS/TS to Covenant IR → Validate effects → Human review

### 3. Specification-to-Code
Requirements + tests → Generate implementation → Compiler validates coverage

### 4. Iterative Refinement
Generate → Compile → Fix errors → Repeat until success

## User Interface

### Command-Line Interface

The system shall provide the following CLI commands:

```bash
# Run evaluation
python run_evaluation.py [--sample N] [--category TYPE] [--provider PROVIDER]

# Analyze results
python run_evaluation.py --analyze RESULTS_FILE

# Select examples
python example_selector.py TASK_DESCRIPTION

# Validate code
python compiler_validator.py FILE.cov

# Interactive quickstart
./quickstart.sh
```

### Programmatic API

The system shall expose Python APIs:

```python
# Generation harness
from generation_harness import GenerationHarness, GenerationTask
harness = GenerationHarness(model_provider=ModelProvider.ANTHROPIC)
metrics = harness.generate(task)

# Example selection
from example_selector import ExampleSelector
selector = ExampleSelector()
examples = selector.select(task_type, max_tokens=1500)

# Compiler validation
from compiler_validator import CompilerValidator
validator = CompilerValidator()
result = validator.validate(source_code)
```

## Implementation Phases

### Phase 1: Proof of Concept (Implemented)

**Status**: ✅ Complete

**Components**:
- Condensed specification document (2,800 tokens)
- Prompt templates (10 templates)
- Example selector with 15 categorized examples
- Compiler integration validator
- Generation harness with self-correction
- Test suite with 105 tasks
- Evaluation runner with analysis reports
- Comprehensive documentation

**Deliverables**:
- `llm-context/` directory with all components
- CLI tools for testing and evaluation
- Mock mode for testing without API costs
- Ready for real API evaluation

### Phase 2: Alpha Deployment (Future)

**Goal**: Deploy to limited users and collect real-world data

**Components**:
- IDE integration (VS Code extension)
- Error recovery loop UI
- Telemetry system
- User feedback collection

**Target**: 10-20 internal users, 1k-5k generations

### Phase 3: Data Collection (Future)

**Goal**: Build training corpus for potential fine-tuning

**Components**:
- Generation labeling system
- Synthetic data generation
- Automated evaluation harness
- Quality metrics tracking

**Target**: 5k-10k curated examples

### Phase 4: Production Decision (Future)

**Goal**: Decide between frontier models and fine-tuned models

**Decision criteria**:
- Monthly generation volume
- Language stability
- Quality requirements
- Budget constraints

### Phase 5: Fine-Tuned Deployment (Future)

**Goal**: Deploy production fine-tuned model (if applicable)

**Components**:
- Synthetic data generation (30k+ examples)
- 13B model fine-tuning with LoRA
- Hybrid system (fine-tuned primary, frontier fallback)
- Gradual traffic migration

## Quality Assurance

### Testing Requirements

The system shall be tested through:

1. **Mock mode testing**: Validate pipeline without API costs
2. **Sample evaluations**: 10-20 task subsets for quick validation
3. **Full suite evaluations**: 100+ task comprehensive testing
4. **Category-specific testing**: Focused testing per task type

### Metrics Collection

All evaluations shall collect:
- Per-task success rates
- Token usage statistics
- Cost breakdowns
- Timing measurements
- Error frequency distributions
- Compiler phase reached
- Correction attempt counts

### Analysis and Reporting

Analysis reports shall provide:
- Success rate trends
- Cost optimization opportunities
- Error pattern identification
- Performance bottleneck detection
- Quality improvement recommendations

## Security and Safety

### API Key Management

- API keys shall be stored in environment variables
- No API keys shall be committed to version control
- Mock mode shall be available for testing without credentials

### Cost Controls

- Real API usage shall require explicit user confirmation
- Cost estimates shall be displayed before execution
- Per-task cost tracking shall prevent runaway expenses

### Code Validation

- All generated code shall pass through compiler validation
- Validation results shall be reported to users
- Failed validations shall not be deployed without user review

## Documentation Requirements

The system shall maintain:

1. **README**: Quick start guide, component overview, examples
2. **Implementation summary**: Detailed component descriptions, metrics
3. **Prompt templates**: Template documentation with usage examples
4. **API documentation**: Programmatic interface specifications
5. **Troubleshooting guide**: Common issues and resolutions

## Extensibility

### Adding LLM Providers

New providers shall be added by:
1. Implementing provider interface in `generation_harness.py`
2. Adding API client initialization
3. Implementing prompt/response formatting
4. Adding provider to CLI options

### Adding Task Categories

New task categories shall be added by:
1. Defining category in `TaskType` enum
2. Cataloging relevant examples
3. Adding category relationships
4. Creating task generators in test suite

### Adding Prompt Templates

New templates shall include:
1. Template structure documentation
2. Parameter specifications
3. Example instantiations
4. Context size budgets
5. Usage guidelines

## Success Metrics

### Phase 1 Validation

Phase 1 shall be considered successful when:
- First-pass success rate exceeds 70%
- Final success rate exceeds 85%
- Average cost per generation is below $0.30
- Average latency is below 30 seconds

### Long-Term Goals

The system shall evolve toward:
- First-pass success rate >80%
- Final success rate >95%
- Cost optimization through caching and prompt engineering
- Sub-second latency for simple tasks (with fine-tuned models)
- Support for 1,000+ generations/day

## Maintenance and Evolution

### Specification Updates

The condensed specification shall be updated when:
- Language syntax changes
- New language features are added
- Error patterns evolve
- Best practices are identified

### Example Catalog Updates

The example catalog shall be updated when:
- New language patterns emerge
- Better representative examples are identified
- Coverage gaps are discovered
- Example quality improvements are available

### Test Suite Updates

The test suite shall be expanded when:
- New language features are added
- Edge cases are discovered
- Coverage analysis identifies gaps
- Real-world usage patterns emerge

---

## References

**Related Documentation**:
- [DESIGN.md](../design/DESIGN.md) - Covenant language design
- [grammar.ebnf](../design/grammar.ebnf) - Formal grammar specification
- [ERROR_CODES.md](../design/ERROR_CODES.md) - Compiler error catalog
- [QUERY_SEMANTICS.md](../design/QUERY_SEMANTICS.md) - Query system semantics

**Implementation Files**:
- [llm-context/](../../llm-context/) - Complete implementation
- [llm-context/README.md](../../llm-context/README.md) - Usage guide
- [llm-context/IMPLEMENTATION_SUMMARY.md](../../llm-context/IMPLEMENTATION_SUMMARY.md) - Implementation details

**Original Analysis**:
- [Feasibility Analysis](../../.claude/implemented_plans/llm-generation-feasibility-analysis.md) - Archived plan

---

**Specification Version**: 1.0
**Implementation Status**: Phase 1 Complete
**Last Reviewed**: 2026-01-11
