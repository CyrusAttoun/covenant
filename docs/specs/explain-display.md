# Explain Display Specification

How explanations are rendered for users across different contexts.

---

## Overview

Explanations can be displayed in multiple formats depending on context:
- CLI output
- IDE hover tooltips
- Generated documentation
- Code review comments

Each format has different space constraints and use cases.

---

## CLI Format

For `covenant explain <snippet_id>` command.

### Standard Output

```
================================================================================
Function: auth.login
================================================================================

SUMMARY
  Authenticates a user with email and password, returning a JWT token on
  success or an error on failure.

SIGNATURE
  fn login(email: String, password: String) -> AuthToken | AuthError

PARAMETERS
  email    : String    - The user's email address for identification
  password : String    - The user's password (plaintext, will be hashed)

RETURNS
  AuthToken  - JWT token containing user claims (on success)
  AuthError  - Authentication failure reason (on failure)

EFFECTS
  database - Reads user record from users table

STEPS
  s1: Query database for user with matching email
      -> Optional<User>

  s2: Verify password against stored bcrypt hash
      -> Bool

  s3: Branch on verification result
      s3a: Generate JWT token with user ID
           -> AuthToken
      s3b: Return InvalidCredentials error
           -> AuthError

REQUIREMENTS
  R-AUTH-001: Users must authenticate with email and password

RELATED
  auth.generate_token (calls)
  docs.auth_overview (documented by)

================================================================================
```

### Compact Output (--compact flag)

```
auth.login(email: String, password: String) -> AuthToken | AuthError

Authenticates user with email/password, returns JWT on success.

Effects: database (read)
Steps: query user -> verify password -> generate token or return error
```

### JSON Output (--json flag)

Outputs raw explanation JSON per schema.

---

## IDE Hover Format

For language server hover tooltips. Limited space.

### Short Hover (default)

```
login(email: String, password: String) -> AuthToken | AuthError

Authenticates user with email and password, returns JWT token.

Effects: database
```

### Extended Hover (on expand)

```
auth.login

Authenticates a user with email and password, returning a JWT token
on success or an AuthError on failure.

Parameters:
  email    - User's email address for identification
  password - User's password (will be verified against bcrypt hash)

Returns:
  AuthToken  - Success: JWT with user claims
  AuthError  - Failure: InvalidCredentials or UserNotFound

Effects:
  database - Reads from users table

Steps:
  s1: Query user by email
  s2: Verify password hash
  s3: Generate token or return error
```

---

## Documentation Format

For generated markdown documentation.

### Function Documentation

```markdown
## auth.login

Authenticates a user with email and password.

### Signature

\`\`\`
fn login(email: String, password: String) -> AuthToken | AuthError
\`\`\`

### Description

Authenticates a user by verifying their email and password against the
database. On successful authentication, generates and returns a JWT token
containing the user's claims. On failure, returns an appropriate error.

### Parameters

| Name | Type | Description |
|------|------|-------------|
| `email` | `String` | The user's email address for identification |
| `password` | `String` | The user's password (plaintext, will be hashed) |

### Returns

| Type | Condition | Description |
|------|-----------|-------------|
| `AuthToken` | Success | JWT token containing user ID and claims |
| `AuthError` | Failure | Error indicating why authentication failed |

### Effects

- **database** - Reads user record from the users table

### Implementation

<details>
<summary>Step-by-step breakdown</summary>

1. **s1**: Query the database for a user matching the provided email
2. **s2**: Verify the provided password against the stored bcrypt hash
3. **s3**: If valid, generate JWT token; otherwise return error

</details>

### Requirements

- [R-AUTH-001](../requirements.md#R-AUTH-001): Users must authenticate with email and password

### See Also

- [auth.generate_token](auth.generate_token.md) - Token generation
- [auth.validate_token](auth.validate_token.md) - Token validation
```

---

## Code Review Format

For PR comments and code review tools.

### Inline Comment

```
// [AI Explanation]
// This function authenticates users by:
// 1. Looking up user by email in database
// 2. Verifying password with bcrypt
// 3. Returning JWT token or error
//
// Effects: database (read-only)
// Returns: AuthToken on success, AuthError on failure
```

### PR Summary

```markdown
### Changes to auth.login

**Before:** N/A (new function)

**After:**
- Authenticates users with email/password
- Returns JWT token on success
- Uses bcrypt for password verification
- Requires `database` effect

**Impact:**
- Adds new authentication capability
- Implements requirement R-AUTH-001
```

---

## Rendering Rules

### Truncation

| Context | Max Summary | Max Description | Max Steps |
|---------|-------------|-----------------|-----------|
| CLI standard | 500 chars | unlimited | all |
| CLI compact | 100 chars | 200 chars | summary only |
| IDE hover short | 100 chars | none | none |
| IDE hover extended | 200 chars | 500 chars | 10 |
| Documentation | unlimited | unlimited | all |
| Code review | 150 chars | 300 chars | 5 |

### Type Formatting

| Context | Format |
|---------|--------|
| CLI | `AuthToken \| AuthError` |
| IDE | `AuthToken | AuthError` |
| Markdown | `` `AuthToken` `` or `` `AuthError` `` |

### Effect Formatting

| Context | Format |
|---------|--------|
| CLI | `database - Reads user record` |
| IDE compact | `database` |
| IDE extended | `database - Reads from users table` |
| Markdown | `**database** - Reads user record` |

---

## Configuration

### User Preferences

```json
{
  "explain.cli.format": "standard",
  "explain.ide.defaultExpanded": false,
  "explain.ide.showEffects": true,
  "explain.ide.showSteps": false,
  "explain.docs.includeSteps": true,
  "explain.docs.includeRequirements": true
}
```

### Per-Project Overrides

In `.covenant/config.json`:

```json
{
  "explain": {
    "defaultVerbosity": "detailed",
    "includeRelated": true,
    "maxStepsInHover": 5
  }
}
```
