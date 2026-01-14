# Generated Comments Specification

Where and how auto-generated comments are injected into Covenant code.

---

## Overview

The comment generator adds human-readable explanations to Covenant code. Comments help developers understand AI-generated code without consulting external documentation.

---

## Comment Types

### Snippet-Level Comments

Appear before the snippet declaration. Provide overview and context.

```
// AUTO-GENERATED EXPLANATION
// =========================
// Function: Authenticates user with email and password
// Effects: database (reads users table)
// Returns: AuthToken on success, AuthError on failure
//
// This function validates user credentials against the database,
// using bcrypt for password verification. On success, it generates
// a JWT token containing the user's ID and claims.

snippet id="auth.login" kind="fn"
```

### Section-Level Comments

Appear before each section to explain its purpose.

```
// EFFECTS: Declares required capabilities
// This function needs database access to look up user records
effects
  effect database
end

// SIGNATURE: Public interface
// Takes email and password, returns either a token or an error
signature
  fn name="login"
    param name="email" type="String"
    param name="password" type="String"
    returns union
      type="AuthToken"
      type="AuthError"
    end
  end
end
```

### Step-Level Comments

Appear before individual steps to explain what they do and why.

```
body
  // Query the users table for a record matching the provided email.
  // Returns Optional<User> - will be None if no user found.
  step id="s1" kind="query"
    target="app_db"
    select all
    from="users"
    where
      equals field="email" var="email"
    end
    limit=1
    as="user_result"
  end

  // Verify the provided password against the stored bcrypt hash.
  // crypto.verify_bcrypt is a pure function (no effects needed).
  step id="s2" kind="call"
    fn="crypto.verify_bcrypt"
    arg name="hash" from="user_result.password_hash"
    arg name="password" from="password"
    as="is_valid"
  end

  // Branch based on password verification result.
  // Success path: generate JWT token
  // Failure path: return InvalidCredentials error
  step id="s3" kind="if"
    condition="is_valid"
    then
      // Generate JWT token with user ID embedded in claims
      step id="s3a" kind="call"
        fn="auth.generate_token"
        arg name="user_id" from="user_result.id"
        as="token"
      end
      step id="s3b" kind="return"
        from="token"
        as="_"
      end
    end
    else
      // Return error indicating invalid credentials
      step id="s3c" kind="return"
        from="AuthError.InvalidCredentials"
        as="_"
      end
    end
    as="_"
  end
end
```

---

## Injection Points

### Before Snippet

```
// AUTO-GENERATED EXPLANATION
// [summary]
// [effects summary]
// [return description]

snippet id="..." kind="..."
```

### Before Effects Section

```
// EFFECTS: [brief description of capabilities needed]
effects
```

### Before Requires Section

```
// REQUIREMENTS: [what specs this implements]
requires
```

### Before Signature Section

```
// SIGNATURE: [interface summary]
signature
```

### Before Body Section

```
// IMPLEMENTATION: [algorithm overview]
body
```

### Before Each Step

```
  // [what this step does]
  // [why it's needed, if non-obvious]
  step id="..." kind="..."
```

### Before Tests Section

```
// TESTS: [test coverage summary]
tests
```

---

## Comment Formatting

### Header Format

```
// AUTO-GENERATED EXPLANATION
// =========================
```

### Section Label Format

```
// EFFECTS: brief description
// SIGNATURE: brief description
// IMPLEMENTATION: brief description
```

### Step Comment Format

```
  // [What] - imperative sentence describing action
  // [Why] - optional, explains rationale
  // [Produces] - type annotation: -> TypeName
```

### Multi-Line Comments

For longer explanations:

```
// This function implements the OAuth 2.0 authorization code flow.
// It validates the authorization code, exchanges it for tokens,
// and returns an access token for API requests.
//
// The flow:
// 1. Validate authorization code
// 2. Exchange code for tokens
// 3. Store refresh token
// 4. Return access token
```

---

## Verbosity Levels

### Minimal

Only snippet-level summary.

```
// Authenticates user with email and password

snippet id="auth.login" kind="fn"
```

### Standard (Default)

Snippet summary + section headers + key step comments.

```
// AUTO-GENERATED EXPLANATION
// Function: Authenticates user with email and password
// Effects: database
// Returns: AuthToken | AuthError

snippet id="auth.login" kind="fn"

// EFFECTS: Database access for user lookup
effects
  effect database
end

// SIGNATURE: email/password -> token or error
signature
  // ...
end

body
  // Query user by email
  step id="s1" kind="query"
    // ...
  end

  // Verify password
  step id="s2" kind="call"
    // ...
  end

  // Return token or error based on verification
  step id="s3" kind="if"
    // ...
  end
end
```

### Detailed

All sections + all steps + type annotations + rationale.

```
// AUTO-GENERATED EXPLANATION
// =========================
// Function: auth.login
// Purpose: Authenticates a user with email and password
// Effects: database (reads from users table)
// Returns: AuthToken on success, AuthError on failure
//
// This function validates user credentials against the database,
// using bcrypt for secure password verification. On success,
// generates a JWT token with user claims.

snippet id="auth.login" kind="fn"

// EFFECTS SECTION
// ---------------
// Declares that this function requires database access.
// Without this declaration, the query step would cause a compiler error.
effects
  effect database
end

// ... (full comments on every element)
```

---

## Comment Markers

To distinguish auto-generated from manual comments:

### Generated Comment Marker

```
// [AUTO] This comment was generated automatically
```

Or header block:

```
// AUTO-GENERATED EXPLANATION
// Generated by: covenant-explain v1.0
// Generated at: 2024-01-15T10:30:00Z
// Snippet hash: abc123...
```

### Manual Comment Preservation

When updating comments, preserve manually-written comments:

```
// [MANUAL] Important: This must be called before session creation
// [AUTO] Validates user credentials against database
step id="s1" kind="query"
```

---

## Update Semantics

### Full Regeneration

`covenant annotate --force <file>`:
- Removes all `[AUTO]` comments
- Regenerates from current AST
- Preserves `[MANUAL]` comments

### Incremental Update

`covenant annotate --update <file>`:
- Updates only stale comments (snippet hash changed)
- Preserves unchanged comments
- Merges with manual comments

### Strip Comments

`covenant strip-comments <file>`:
- Removes all `[AUTO]` comments
- Preserves `[MANUAL]` comments
- Preserves plain `//` comments without markers

---

## Configuration

### Project Settings

In `.covenant/config.json`:

```json
{
  "comments": {
    "verbosity": "standard",
    "includeHeader": true,
    "includeTimestamp": false,
    "markerStyle": "auto",
    "maxLineLength": 80
  }
}
```

### Per-Snippet Override

Using metadata:

```
metadata
  comment_verbosity="detailed"
end
```

Or suppression:

```
metadata
  no_auto_comments=true
end
```
