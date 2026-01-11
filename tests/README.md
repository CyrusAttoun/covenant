# Covenant Test Suite

## Running Tests

### Using cargo test (default)

```bash
cargo test                                  # Run all tests
cargo test --test parse_all_examples        # Parser tests only
cargo test -p covenant-parser               # Tests for specific crate
cargo test -- --nocapture                   # Show test output (e.g., which examples were parsed)
```

### Using cargo-nextest (optional)

For better output formatting and parallel execution, you can optionally install cargo-nextest:

```bash
cargo install cargo-nextest --locked
```

Then run tests:
```bash
cargo nextest run                           # All tests, parallel execution
cargo nextest run --test parse_all_examples # Parser tests only
cargo nextest run -p covenant-parser        # Tests for specific crate
cargo nextest run --profile ci              # CI profile with retries
```

## Adding New Examples

Simply create a `.cov` file in the `examples/` directory - it will be automatically discovered and tested!

**Example:**
```bash
# Create a new example
touch examples/21-my-feature.cov

# Edit the file with your Covenant code
# ...

# Run tests - your example is auto-discovered!
cargo test --test parse_all_examples
```

The parser test suite uses dynamic discovery, so no test code changes are needed when you add new examples.

## Test Organization

### Unit Tests (in crates/)
- **Location:** In `src/` files as `#[cfg(test)]` modules
- **Purpose:** Test internal implementation details, private functions, edge cases
- **Examples:**
  - [crates/covenant-lexer/src/lib.rs](../crates/covenant-lexer/src/lib.rs) - Lexer unit tests
  - [crates/covenant-parser/src/lib.rs](../crates/covenant-parser/src/lib.rs) - Parser unit tests
  - [crates/covenant-storage/src/sync.rs](../crates/covenant-storage/src/sync.rs) - Storage sync tests

### Integration Tests (in tests/)
- **Location:** `tests/` directory
- **Purpose:** Test public APIs across crates, end-to-end functionality
- **Structure:**
  ```
  tests/
  ├── common/           # Shared test utilities
  ├── parser_tests/     # Parser integration tests
  ├── query_tests/      # Query semantic tests (future)
  ├── codegen_tests/    # Code generation tests (future)
  └── storage_tests/    # Storage integration tests (future)
  ```

## Using Test Utilities

The `common/` module provides shared utilities for tests:

```rust
mod common;

#[test]
fn test_example() {
    // Load an example file
    let source = common::fixtures::load_example("01-hello-world");

    // Assert it parses successfully
    common::assertions::assert_parses(&source);
}
```

### Available Utilities

**Fixtures (`common::fixtures`):**
- `load_example(name)` - Load a .cov file from examples/
- `example_path(name)` - Get path to an example file
- `load_fixture(name)` - Load a test fixture from tests/fixtures/

**Assertions (`common::assertions`):**
- `assert_parses(source)` - Assert that source parses successfully
- `assert_parse_fails(source)` - Assert that source fails to parse
- `assert_has_snippet(program, id)` - Assert AST contains a snippet with given ID

## Current Test Status

**Unit Tests:**
- covenant-lexer: 3 tests
- covenant-parser: 3 tests
- covenant-storage: 8 tests

**Integration Tests:**
- Parser tests: Tests all 19 .cov examples

**Total:** 12+ tests (all passing)

## Future Test Suites

The infrastructure is designed to easily add:

### Query Tests (`tests/query_tests/`)
Test Covenant query semantics and AST queries:
- Validate query operations on project AST
- Test filtering, joining, and aggregations
- Verify SQL dialect handling

### Codegen Tests (`tests/codegen_tests/`)
Test WASM generation:
- Validate IR transformations
- Test WASM output correctness
- Verify effect propagation in codegen

### Storage Tests (`tests/storage_tests/`)
Integration tests for persistence:
- Test file watching and sync
- Validate backend implementations (in-memory, redb)
- Test concurrent access patterns

### E2E Tests (`tests/e2e_tests/`)
Full compiler pipeline:
- Compile examples → execute WASM → validate output
- Test complete workflows
- Integration with external systems

## CI/CD Integration

The project is configured for CI/CD with cargo-nextest:

```yaml
# Example GitHub Actions configuration
- name: Run tests
  run: cargo nextest run --profile ci

- name: Upload test results
  uses: actions/upload-artifact@v3
  with:
    name: test-results
    path: target/nextest/junit.xml
```

The CI profile includes:
- Automatic retries for flaky tests
- JUnit XML output for test reporting
- Extended timeouts for slower CI environments

## Troubleshooting

### Tests not discovering examples
Ensure the `examples/` directory exists and contains `.cov` files with the correct extension.

### cargo-nextest not found
Install it with: `cargo install cargo-nextest --locked`

### Tests failing on Windows
Ensure file paths use the correct separators. The test utilities handle cross-platform paths automatically.

## Contributing

When adding new features:
1. Add unit tests in the relevant crate
2. Add integration tests if testing across crates
3. Add example `.cov` files to demonstrate the feature
4. Run all tests before submitting PR: `cargo test`
