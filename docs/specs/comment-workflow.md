# Comment Workflow Specification

When and how auto-generated comments are created and maintained.

---

## Overview

Comments can be generated at several points in the development workflow:
1. During LLM code generation
2. As a post-processing command
3. Via IDE integration
4. In CI/CD pipelines

---

## Generation Triggers

### 1. During LLM Code Generation

When Claude or another LLM generates Covenant code, comments are generated simultaneously.

**Prompt Instruction:**
```
When generating Covenant code, include inline comments that:
- Explain what each section does
- Describe non-obvious steps
- Note the purpose of each function

Use this format:
// [section explanation]
[section content]

// [step explanation]
step id="..." kind="..."
```

**Configuration:**

In the code generation spec, set comment verbosity:

```json
{
  "generation": {
    "commentVerbosity": "standard",
    "includeStepComments": true,
    "includeSectionComments": true
  }
}
```

### 2. Post-Generation Command

Annotate existing code:

```bash
# Annotate a single file
covenant annotate src/auth.cov

# Annotate with specific verbosity
covenant annotate --verbosity=detailed src/auth.cov

# Annotate all files
covenant annotate --all

# Update stale comments only
covenant annotate --update src/

# Force regeneration
covenant annotate --force src/auth.cov

# Strip auto-generated comments
covenant strip-comments src/auth.cov
```

### 3. IDE Integration

**Code Actions:**
- "Explain this snippet" - Add detailed comments
- "Simplify comments" - Reduce to minimal
- "Update comments" - Refresh stale comments

**Hover Tooltips:**
- Show explanation on hover (without modifying file)
- Option to insert as comment

**Language Server Protocol:**
```json
{
  "command": "covenant.annotate",
  "arguments": {
    "snippetId": "auth.login",
    "verbosity": "standard"
  }
}
```

### 4. Git Pre-Commit Hook

Auto-annotate before commit:

**`.covenant/hooks/pre-commit`:**
```bash
#!/bin/bash
# Auto-annotate staged .cov files

staged_cov_files=$(git diff --cached --name-only --diff-filter=ACM | grep '\.cov$')

if [ -n "$staged_cov_files" ]; then
    covenant annotate --update $staged_cov_files
    git add $staged_cov_files
fi
```

**Enable hook:**
```bash
covenant hooks install pre-commit
```

### 5. CI/CD Pipeline

**GitHub Action:**
```yaml
name: Documentation
on: [push]
jobs:
  annotate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: covenant-lang/setup@v1
      - name: Check comment coverage
        run: covenant annotate --check --coverage=80%
      - name: Generate documentation
        run: covenant docs generate
```

---

## Workflow Scenarios

### Scenario 1: New Function Generation

1. User requests: "Create a user login function"
2. LLM generates code with comments
3. User reviews and commits

```
// [LLM generates this]
// AUTO-GENERATED
// Authenticates user with email and password

snippet id="auth.login" kind="fn"
// ... code with inline comments ...
end
```

### Scenario 2: Existing Code Without Comments

1. Developer runs: `covenant annotate src/legacy.cov`
2. Generator analyzes AST
3. Comments inserted based on analysis
4. Developer reviews diff and commits

### Scenario 3: Updating After Code Change

1. Developer modifies function
2. Pre-commit hook detects change (hash mismatch)
3. Hook runs: `covenant annotate --update <file>`
4. Stale comments regenerated
5. Commit proceeds with updated comments

### Scenario 4: Removing Comments

1. Developer decides comments are cluttering code
2. Runs: `covenant strip-comments --level=step src/auth.cov`
3. Step-level comments removed, section comments preserved
4. Manual comments (`[MANUAL]`) preserved

---

## Comment Staleness Detection

### Hash-Based Detection

Each auto-generated comment block includes a hash:

```
// AUTO-GENERATED (hash: abc123)
// Function: Authenticates user
```

On update:
1. Compute current snippet hash
2. Compare with stored hash
3. If different, mark as stale

### Staleness Indicators

In IDE, stale comments could be:
- Grayed out
- Marked with warning icon
- Shown with "Update available" hint

---

## Comment Preservation

### Manual Comments

Comments marked `[MANUAL]` are never modified:

```
// [MANUAL] IMPORTANT: Rate limit this endpoint!
// [AUTO] Authenticates user with email and password
snippet id="auth.login" kind="fn"
```

### Unmarked Comments

Plain `//` comments (no marker) are:
- Preserved by `--update`
- Removed by `--force`
- Preserved by `strip-comments`

### Best Practice

Encourage markers for clarity:

```
// [MANUAL] Business rule: Max 5 attempts per hour
// [AUTO] Validates login attempt count
step id="s1" kind="query"
```

---

## Integration with Version Control

### Commit Message Hints

When comments are auto-generated, suggest commit message:

```
docs: Auto-generate comments for auth module

- Added comments to auth.login
- Added comments to auth.validate_token
- Updated stale comments in auth.refresh

Generated by: covenant annotate v1.0
```

### Diff View

Show comment changes separately:

```diff
  snippet id="auth.login" kind="fn"

+ // EFFECTS: Database access for user lookup
  effects
    effect database
  end
```

---

## Configuration

### Project Configuration

**`.covenant/config.json`:**
```json
{
  "comments": {
    "autoGenerate": true,
    "verbosity": "standard",
    "preCommitHook": true,
    "ciCheck": true,
    "minCoverage": 80
  }
}
```

### User Configuration

**`~/.covenant/config.json`:**
```json
{
  "comments": {
    "defaultVerbosity": "minimal",
    "preferredMarkerStyle": "simple"
  }
}
```

### Per-File Override

At top of file:

```
// @covenant-comments verbosity=detailed
// @covenant-comments skip-steps=s1,s2

snippet id="complex.function" kind="fn"
```

---

## Coverage Reporting

Track comment coverage:

```bash
covenant annotate --coverage-report

Comment Coverage Report
=======================
Total snippets: 42
With comments:  38 (90%)
Stale comments: 3 (7%)
Missing:        4 (10%)

Missing comments:
  - src/utils.cov: utils.helper_fn
  - src/db.cov: db.raw_query
  - src/db.cov: db.batch_insert
  - src/http.cov: http.middleware
```

---

## Error Handling

### Generation Failures

If comment generation fails:
1. Log error with context
2. Leave code unchanged
3. Report in CLI output
4. Continue with other files

### Parse Errors

If file has syntax errors:
1. Report parse error
2. Skip comment generation for that file
3. Suggest fixing errors first

### LLM Timeouts

If LLM enhancement times out:
1. Fall back to template-based comments
2. Log timeout warning
3. Mark as "template-only" in metadata
