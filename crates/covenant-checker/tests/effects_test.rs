//! Tests for effect checking (Phase 3)

use covenant_checker::{check_effects, EffectError};
use covenant_symbols::build_symbol_graph;
use covenant_parser::parse;

/// Helper to parse and check effects, returning the result
fn check_effects_for_source(source: &str) -> covenant_checker::EffectCheckResult {
    let program = parse(source).expect("parse failed");
    let symbol_result = build_symbol_graph(&program).expect("symbol graph failed");
    check_effects(&symbol_result.graph)
}

#[test]
fn pure_function_no_calls_is_valid() {
    let source = r#"
snippet id="math.add" kind="fn"

signature
  fn name="add"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=add
    input var="a"
    input var="b"
    as="result"
  end

  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Expected no violations");

    let closure = result.closures.get("math.add").expect("closure not found");
    assert!(closure.is_pure, "Expected function to be pure");
    assert!(closure.declared.is_empty(), "Expected no declared effects");
    assert!(closure.computed.is_empty(), "Expected no computed effects");
}

#[test]
fn pure_calls_pure_is_valid() {
    let source = r#"
snippet id="math.double" kind="fn"

signature
  fn name="double"
    param name="x" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=add
    input var="x"
    input var="x"
    as="result"
  end

  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end

snippet id="math.quadruple" kind="fn"

signature
  fn name="quadruple"
    param name="x" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="call"
    fn="math.double"
    arg name="x" from="x"
    as="doubled"
  end

  step id="s2" kind="call"
    fn="math.double"
    arg name="x" from="doubled"
    as="result"
  end

  step id="s3" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Expected no violations");

    // Both functions should be pure
    let double_closure = result.closures.get("math.double").expect("double closure not found");
    assert!(double_closure.is_pure);

    let quad_closure = result.closures.get("math.quadruple").expect("quadruple closure not found");
    assert!(quad_closure.is_pure);
    assert!(quad_closure.computed.is_empty(), "Expected no computed effects for quadruple");
}

#[test]
fn pure_calls_effectful_is_error() {
    let source = r#"
snippet id="io.print" kind="extern"

effects
  effect console
end

signature
  fn name="print"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end

snippet id="greet" kind="fn"

signature
  fn name="greet"
    param name="name" type="String"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="io.print"
    arg name="msg" from="name"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert_eq!(result.violations.len(), 1, "Expected 1 violation");

    match &result.violations[0] {
        EffectError::PureCallsEffectful { function, callee, effects, .. } => {
            assert_eq!(function, "greet");
            assert_eq!(callee, "io.print");
            assert!(effects.contains(&"console".to_string()));
        }
        _ => panic!("Expected PureCallsEffectful error"),
    }
}

#[test]
fn effectful_with_correct_declaration_is_valid() {
    let source = r#"
snippet id="io.print" kind="extern"

effects
  effect console
end

signature
  fn name="print"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end

snippet id="greet" kind="fn"

effects
  effect console
end

signature
  fn name="greet"
    param name="name" type="String"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="io.print"
    arg name="msg" from="name"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Expected no violations");

    let closure = result.closures.get("greet").expect("greet closure not found");
    assert!(!closure.is_pure);
    assert!(closure.declared.contains("console"));
    assert!(closure.computed.contains("console"));
}

#[test]
fn effectful_missing_declaration_is_error() {
    let source = r#"
snippet id="io.print" kind="extern"

effects
  effect console
end

signature
  fn name="print"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end

snippet id="io.read" kind="extern"

effects
  effect filesystem
end

signature
  fn name="read"
    param name="path" type="String"
    returns type="String"
  end
end

end

snippet id="process" kind="fn"

effects
  effect console
end

signature
  fn name="process"
    param name="path" type="String"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="io.read"
    arg name="path" from="path"
    as="content"
  end

  step id="s2" kind="call"
    fn="io.print"
    arg name="msg" from="content"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert_eq!(result.violations.len(), 1, "Expected 1 violation");

    match &result.violations[0] {
        EffectError::MissingEffect { function, missing, source_callee, .. } => {
            assert_eq!(function, "process");
            assert!(missing.contains(&"filesystem".to_string()));
            assert_eq!(source_callee, "io.read");
        }
        _ => panic!("Expected MissingEffect error"),
    }
}

#[test]
fn transitive_effects_must_be_declared() {
    let source = r#"
snippet id="io.print" kind="extern"

effects
  effect console
end

signature
  fn name="print"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end

snippet id="logger.log" kind="fn"

effects
  effect console
end

signature
  fn name="log"
    param name="msg" type="String"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="io.print"
    arg name="msg" from="msg"
    as="_"
  end
end

end

snippet id="app.run" kind="fn"

signature
  fn name="run"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="logger.log"
    arg name="msg" lit="Starting app"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert_eq!(result.violations.len(), 1, "Expected 1 violation for transitive effect");

    match &result.violations[0] {
        EffectError::PureCallsEffectful { function, callee, effects, .. } => {
            assert_eq!(function, "app.run");
            assert_eq!(callee, "logger.log");
            assert!(effects.contains(&"console".to_string()));
        }
        _ => panic!("Expected PureCallsEffectful error"),
    }

    // Verify closure computation
    let closure = result.closures.get("app.run").expect("app.run closure not found");
    assert!(closure.is_pure, "app.run declares no effects");
    assert!(closure.computed.contains("console"), "app.run transitively uses console");
}

#[test]
fn multiple_effects_all_must_be_declared() {
    let source = r#"
snippet id="io.print" kind="extern"

effects
  effect console
end

signature
  fn name="print"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end

snippet id="io.read" kind="extern"

effects
  effect filesystem
end

signature
  fn name="read"
    param name="path" type="String"
    returns type="String"
  end
end

end

snippet id="net.fetch" kind="extern"

effects
  effect network
end

signature
  fn name="fetch"
    param name="url" type="String"
    returns type="String"
  end
end

end

snippet id="app.main" kind="fn"

effects
  effect console
  effect filesystem
  effect network
end

signature
  fn name="main"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="io.read"
    arg name="path" lit="config.txt"
    as="config"
  end

  step id="s2" kind="call"
    fn="net.fetch"
    arg name="url" lit="https://example.com"
    as="data"
  end

  step id="s3" kind="call"
    fn="io.print"
    arg name="msg" from="data"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Expected no violations when all effects declared");

    let closure = result.closures.get("app.main").expect("app.main closure not found");
    assert!(!closure.is_pure);
    assert_eq!(closure.declared.len(), 3);
    assert_eq!(closure.computed.len(), 3);
    assert!(closure.computed.contains("console"));
    assert!(closure.computed.contains("filesystem"));
    assert!(closure.computed.contains("network"));
}

// Note: Cyclic/recursive calls are rejected at Phase 2 (symbol graph building)
// with the CircularImport error. So we don't need to test cycle handling in
// the effect checker - it will never receive cyclic graphs.

#[test]
fn extern_effects_propagate_correctly() {
    // Verify that extern functions correctly propagate their effects
    let source = r#"
snippet id="crypto.sha256" kind="extern"

signature
  fn name="sha256"
    param name="input" type="String"
    returns type="String"
  end
end

end

snippet id="hash.hash_password" kind="fn"

signature
  fn name="hash_password"
    param name="password" type="String"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="crypto.sha256"
    arg name="input" from="password"
    as="result"
  end

  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);

    // crypto.sha256 is pure (no effects declared), so hash_password calling it is valid
    assert!(result.violations.is_empty(), "Expected no violations when calling pure extern");

    let sha_closure = result.closures.get("crypto.sha256").expect("sha256 closure not found");
    assert!(sha_closure.is_pure, "sha256 should be pure");

    let hash_closure = result.closures.get("hash.hash_password").expect("hash_password closure not found");
    assert!(hash_closure.is_pure, "hash_password should be pure since it only calls pure functions");
}

// ==========================================================================
// COMPREHENSIVE PHASE 3 TESTS - Effect Closure, Diamond Dependencies, etc.
// ==========================================================================

#[test]
fn effect_closure_three_level_chain() {
    // A -> B -> C where C has database effect
    // Both A and B should have database in their computed closures
    let source = r#"
snippet id="db.query" kind="extern"

effects
  effect database
end

signature
  fn name="query"
    param name="sql" type="String"
    returns type="String"
  end
end

end

snippet id="repo.get_user" kind="fn"

effects
  effect database
end

signature
  fn name="get_user"
    param name="id" type="Int"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="db.query"
    arg name="sql" lit="SELECT * FROM users"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end

snippet id="service.fetch_user" kind="fn"

effects
  effect database
end

signature
  fn name="fetch_user"
    param name="id" type="Int"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="repo.get_user"
    arg name="id" from="id"
    as="user"
  end
  step id="s2" kind="return"
    from="user"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Expected no violations with proper effect declarations");

    // All three should have database in computed closure
    let db_closure = result.closures.get("db.query").expect("db.query closure");
    let repo_closure = result.closures.get("repo.get_user").expect("repo.get_user closure");
    let service_closure = result.closures.get("service.fetch_user").expect("service.fetch_user closure");

    assert!(db_closure.computed.contains("database"));
    assert!(repo_closure.computed.contains("database"));
    assert!(service_closure.computed.contains("database"));
}

#[test]
fn effect_closure_diamond_dependency() {
    // A calls B and C, both call D which has effect
    // A needs the effect even though it reaches D through two paths
    let source = r#"
snippet id="io.log" kind="extern"

effects
  effect console
end

signature
  fn name="log"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end

snippet id="logger.info" kind="fn"

effects
  effect console
end

signature
  fn name="info"
    param name="msg" type="String"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="io.log"
    arg name="msg" from="msg"
    as="_"
  end
end

end

snippet id="logger.debug" kind="fn"

effects
  effect console
end

signature
  fn name="debug"
    param name="msg" type="String"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="io.log"
    arg name="msg" from="msg"
    as="_"
  end
end

end

snippet id="app.main" kind="fn"

effects
  effect console
end

signature
  fn name="main"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="logger.info"
    arg name="msg" lit="Starting"
    as="_"
  end
  step id="s2" kind="call"
    fn="logger.debug"
    arg name="msg" lit="Debug info"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Diamond dependency should work with proper declarations");

    let main_closure = result.closures.get("app.main").expect("app.main closure");
    assert!(main_closure.computed.contains("console"));
}

#[test]
fn effect_missing_in_chain() {
    // A -> B -> C where C has effect, B declares it, but A doesn't
    let source = r#"
snippet id="io.write" kind="extern"

effects
  effect filesystem
end

signature
  fn name="write"
    param name="path" type="String"
    param name="data" type="String"
    returns type="Bool"
  end
end

end

snippet id="file.save" kind="fn"

effects
  effect filesystem
end

signature
  fn name="save"
    param name="name" type="String"
    param name="content" type="String"
    returns type="Bool"
  end
end

body
  step id="s1" kind="call"
    fn="io.write"
    arg name="path" from="name"
    arg name="data" from="content"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end

snippet id="app.export" kind="fn"

signature
  fn name="export"
    param name="data" type="String"
    returns type="Bool"
  end
end

body
  step id="s1" kind="call"
    fn="file.save"
    arg name="name" lit="export.txt"
    arg name="content" from="data"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(!result.violations.is_empty(), "Should detect missing effect in chain");

    // app.export should have filesystem in computed but not declared
    let export_closure = result.closures.get("app.export").expect("app.export closure");
    assert!(export_closure.computed.contains("filesystem"));
    assert!(!export_closure.declared.contains("filesystem"));
}

#[test]
fn effect_multiple_missing() {
    // Function calls two effectful functions but declares neither
    let source = r#"
snippet id="io.read" kind="extern"

effects
  effect filesystem
end

signature
  fn name="read"
    param name="path" type="String"
    returns type="String"
  end
end

end

snippet id="net.post" kind="extern"

effects
  effect network
end

signature
  fn name="post"
    param name="url" type="String"
    param name="body" type="String"
    returns type="String"
  end
end

end

snippet id="app.upload" kind="fn"

signature
  fn name="upload"
    param name="path" type="String"
    param name="url" type="String"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="io.read"
    arg name="path" from="path"
    as="content"
  end
  step id="s2" kind="call"
    fn="net.post"
    arg name="url" from="url"
    arg name="body" from="content"
    as="result"
  end
  step id="s3" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);

    // Should have violations for both missing effects
    assert!(!result.violations.is_empty(), "Should detect missing effects");

    let upload_closure = result.closures.get("app.upload").expect("app.upload closure");
    assert!(upload_closure.computed.contains("filesystem"));
    assert!(upload_closure.computed.contains("network"));
    assert!(upload_closure.declared.is_empty());
}

#[test]
fn effect_superset_declaration_valid() {
    // Declaring more effects than needed is OK (overapproximation)
    let source = r#"
snippet id="pure.add" kind="fn"

signature
  fn name="add"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=add
    input var="a"
    input var="b"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end

snippet id="app.calculate" kind="fn"

effects
  effect console
  effect network
end

signature
  fn name="calculate"
    param name="x" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="call"
    fn="pure.add"
    arg name="a" from="x"
    arg name="b" lit=1
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    // Over-declaration should be allowed (conservative)
    assert!(result.violations.is_empty(), "Over-declaring effects should be valid");

    let calc_closure = result.closures.get("app.calculate").expect("app.calculate closure");
    // Declared has console and network
    assert!(calc_closure.declared.contains("console"));
    assert!(calc_closure.declared.contains("network"));
    // The computed closure includes the function's own declared effects
    // The key point is that there are no violations - over-declaring is fine
}

#[test]
fn effect_partial_declaration_error() {
    // Declaring some effects but missing others
    let source = r#"
snippet id="io.log" kind="extern"

effects
  effect console
end

signature
  fn name="log"
    param name="msg" type="String"
    returns type="Unit"
  end
end

end

snippet id="db.query" kind="extern"

effects
  effect database
end

signature
  fn name="query"
    param name="sql" type="String"
    returns type="String"
  end
end

end

snippet id="app.process" kind="fn"

effects
  effect console
end

signature
  fn name="process"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="io.log"
    arg name="msg" lit="Processing"
    as="_"
  end
  step id="s2" kind="call"
    fn="db.query"
    arg name="sql" lit="SELECT 1"
    as="result"
  end
  step id="s3" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(!result.violations.is_empty(), "Should detect missing database effect");

    // Check that the violation mentions the missing effect
    match &result.violations[0] {
        EffectError::MissingEffect { function, missing, .. } => {
            assert_eq!(function, "app.process");
            assert!(missing.contains(&"database".to_string()));
        }
        _ => panic!("Expected MissingEffect error"),
    }
}

#[test]
fn effect_empty_function_is_pure() {
    // Function with no calls and no effects is pure
    let source = r#"
snippet id="const.answer" kind="fn"

signature
  fn name="answer"
    returns type="Int"
  end
end

body
  step id="s1" kind="return"
    lit=42
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty());

    let closure = result.closures.get("const.answer").expect("closure");
    assert!(closure.is_pure);
    assert!(closure.declared.is_empty());
    assert!(closure.computed.is_empty());
}

#[test]
fn effect_only_computes_is_pure() {
    // Function with only compute steps (no calls) is pure
    let source = r#"
snippet id="math.complex" kind="fn"

signature
  fn name="complex"
    param name="a" type="Int"
    param name="b" type="Int"
    param name="c" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=add
    input var="a"
    input var="b"
    as="sum"
  end
  step id="s2" kind="compute"
    op=mul
    input var="sum"
    input var="c"
    as="result"
  end
  step id="s3" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty());

    let closure = result.closures.get("math.complex").expect("closure");
    assert!(closure.is_pure);
}

#[test]
fn effect_database_in_query_step() {
    // Query steps implicitly require database effect
    let source = r#"
snippet id="db.main_db" kind="database"

metadata
  type="database"
  dialect="postgres"
end

end

snippet id="data.get_users" kind="fn"

effects
  effect database
end

signature
  fn name="get_users"
    returns collection of="User"
  end
end

body
  step id="s1" kind="query"
    target="project"
    select all
    from="users"
    as="users"
  end
  step id="s2" kind="return"
    from="users"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    // Should be valid with database effect declared
    assert!(result.violations.is_empty(), "Query with database effect should be valid");
}

// ==========================================================================
// DOTTED EFFECT NAMES - Namespaced effects are independent strings
// ==========================================================================

#[test]
fn dotted_effect_names_propagate_correctly() {
    // Dotted effect names like "std.storage" work as regular string effects
    let source = r#"
snippet id="std.storage.kv.set" kind="extern"

effects
  effect std.storage
end

signature
  fn name="set"
    param name="key" type="String"
    param name="value" type="String"
    returns type="Unit"
  end
end

end

snippet id="app.save_config" kind="fn"

effects
  effect std.storage
end

signature
  fn name="save_config"
    param name="value" type="String"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="std.storage.kv.set"
    arg name="key" lit="config"
    arg name="value" from="value"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Matching dotted effects should be valid");

    let closure = result.closures.get("app.save_config").expect("closure not found");
    assert!(closure.declared.contains("std.storage"));
    assert!(closure.computed.contains("std.storage"));
}

#[test]
fn dotted_effects_are_independent_strings() {
    // "database" and "database.read" are completely independent effects.
    // Declaring "database" does NOT cover "database.read"
    let source = r#"
snippet id="db.reader" kind="extern"

effects
  effect database.read
end

signature
  fn name="read"
    param name="query" type="String"
    returns type="String"
  end
end

end

snippet id="app.fetch" kind="fn"

effects
  effect database
end

signature
  fn name="fetch"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="db.reader"
    arg name="query" lit="SELECT 1"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    // "database" does NOT subsume "database.read" — they are independent
    assert_eq!(result.violations.len(), 1, "Expected violation: database != database.read");

    match &result.violations[0] {
        EffectError::MissingEffect { function, missing, .. } => {
            assert_eq!(function, "app.fetch");
            assert!(missing.contains(&"database.read".to_string()));
        }
        _ => panic!("Expected MissingEffect error"),
    }
}

#[test]
fn multi_level_dotted_effects_work() {
    // Effects like "network.http.get" work as independent strings
    let source = r#"
snippet id="http.get" kind="extern"

effects
  effect network.http.get
end

signature
  fn name="get"
    param name="url" type="String"
    returns type="String"
  end
end

end

snippet id="api.fetch_users" kind="fn"

effects
  effect network.http.get
end

signature
  fn name="fetch_users"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="http.get"
    arg name="url" lit="https://api.example.com/users"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Multi-level dotted effect should work");

    let closure = result.closures.get("api.fetch_users").expect("closure not found");
    assert!(closure.declared.contains("network.http.get"));
    assert!(closure.computed.contains("network.http.get"));
}

// ==========================================================================
// PARAMETERIZED EFFECTS - Priority 2 Feature
// ==========================================================================

#[test]
fn parameterized_effect_parses_correctly() {
    // Basic test: verify parameterized effects parse without error
    let source = r#"
snippet id="fs.read_data" kind="extern"

effects
  effect filesystem(path="/data")
end

signature
  fn name="read_data"
    returns type="String"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(), "Parameterized effect should parse correctly");

    let closure = result.closures.get("fs.read_data").expect("closure not found");
    assert!(closure.declared.contains("filesystem"));
    assert!(!closure.declared_full.is_empty());

    // Check that the effect has a path parameter
    let fs_effect = closure.declared_full.iter()
        .find(|e| e.name == "filesystem")
        .expect("filesystem effect not found");
    assert!(fs_effect.has_params());
    let path_param = fs_effect.get_param("path").expect("path param not found");
    assert_eq!(path_param.value, covenant_ast::Literal::String("/data".to_string()));
}

#[test]
fn parameterized_effect_without_params_covers_all() {
    // If declared effect has no params, it covers any parameterized version (wildcard)
    let source = r#"
snippet id="fs.read_specific" kind="extern"

effects
  effect filesystem(path="/data/users")
end

signature
  fn name="read_specific"
    returns type="String"
  end
end

end

snippet id="app.process" kind="fn"

effects
  effect filesystem
end

signature
  fn name="process"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="fs.read_specific"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(),
        "Effect without params should cover parameterized version (wildcard)");
}

#[test]
fn parameterized_effect_path_prefix_subsumes() {
    // Declared path="/data" should subsume required path="/data/users"
    let source = r#"
snippet id="fs.read_users" kind="extern"

effects
  effect filesystem(path="/data/users")
end

signature
  fn name="read_users"
    returns type="String"
  end
end

end

snippet id="app.get_users" kind="fn"

effects
  effect filesystem(path="/data")
end

signature
  fn name="get_users"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="fs.read_users"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(),
        "Path /data should subsume /data/users (prefix match)");
}

#[test]
fn parameterized_effect_path_not_prefix_fails() {
    // Declared path="/other" should NOT subsume required path="/data/users"
    let source = r#"
snippet id="fs.read_users" kind="extern"

effects
  effect filesystem(path="/data/users")
end

signature
  fn name="read_users"
    returns type="String"
  end
end

end

snippet id="app.get_users" kind="fn"

effects
  effect filesystem(path="/other")
end

signature
  fn name="get_users"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="fs.read_users"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert_eq!(result.violations.len(), 1,
        "Path /other should NOT subsume /data/users");

    match &result.violations[0] {
        EffectError::ParameterNotCovered {
            function, effect_name, param_name, required_value, declared_value, ..
        } => {
            assert_eq!(function, "app.get_users");
            assert_eq!(effect_name, "filesystem");
            assert_eq!(param_name, "path");
            assert_eq!(required_value, "\"/data/users\"");
            assert_eq!(declared_value.as_ref().unwrap(), "\"/other\"");
        }
        _ => panic!("Expected ParameterNotCovered error, got {:?}", result.violations[0]),
    }
}

#[test]
fn parameterized_effect_multiple_params() {
    // Test with multiple parameters
    // Note: "table" is a keyword, so we use "target" instead
    let source = r#"
snippet id="db.query_users" kind="extern"

effects
  effect database(target="users", readonly=true)
end

signature
  fn name="query_users"
    returns type="String"
  end
end

end

snippet id="app.list_users" kind="fn"

effects
  effect database(target="users", readonly=true)
end

signature
  fn name="list_users"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="db.query_users"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(),
        "Matching multiple params should work");

    let closure = result.closures.get("db.query_users").expect("closure not found");
    let db_effect = closure.declared_full.iter()
        .find(|e| e.name == "database")
        .expect("database effect not found");

    assert_eq!(db_effect.params.len(), 2);
}

#[test]
fn parameterized_effect_int_param() {
    // Test with integer parameter
    let source = r#"
snippet id="rate.limited_call" kind="extern"

effects
  effect ratelimit(max=100)
end

signature
  fn name="limited_call"
    returns type="String"
  end
end

end

snippet id="app.call" kind="fn"

effects
  effect ratelimit(max=100)
end

signature
  fn name="call"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="rate.limited_call"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(),
        "Matching integer params should work");
}

#[test]
fn parameterized_effect_int_param_mismatch() {
    // Different integer values should not match
    let source = r#"
snippet id="rate.limited_call" kind="extern"

effects
  effect ratelimit(max=100)
end

signature
  fn name="limited_call"
    returns type="String"
  end
end

end

snippet id="app.call" kind="fn"

effects
  effect ratelimit(max=50)
end

signature
  fn name="call"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="rate.limited_call"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert_eq!(result.violations.len(), 1,
        "Different integer param values should not match");

    match &result.violations[0] {
        EffectError::ParameterNotCovered { param_name, required_value, declared_value, .. } => {
            assert_eq!(param_name, "max");
            assert_eq!(required_value, "100");
            assert_eq!(declared_value.as_ref().unwrap(), "50");
        }
        _ => panic!("Expected ParameterNotCovered error"),
    }
}

#[test]
fn parameterized_effect_transitive() {
    // Parameterized effects should propagate transitively
    let source = r#"
snippet id="fs.read_log" kind="extern"

effects
  effect filesystem(path="/var/log")
end

signature
  fn name="read_log"
    returns type="String"
  end
end

end

snippet id="logger.get_last" kind="fn"

effects
  effect filesystem(path="/var/log")
end

signature
  fn name="get_last"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="fs.read_log"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end

snippet id="app.show_log" kind="fn"

effects
  effect filesystem(path="/var")
end

signature
  fn name="show_log"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="logger.get_last"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(),
        "Transitive parameterized effects should work with path prefix");
}

#[test]
fn parameterized_effect_exact_path_match() {
    // Exact same path should always work
    let source = r#"
snippet id="fs.read_config" kind="extern"

effects
  effect filesystem(path="/etc/config")
end

signature
  fn name="read_config"
    returns type="String"
  end
end

end

snippet id="app.get_config" kind="fn"

effects
  effect filesystem(path="/etc/config")
end

signature
  fn name="get_config"
    returns type="String"
  end
end

body
  step id="s1" kind="call"
    fn="fs.read_config"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end

end
"#;

    let result = check_effects_for_source(source);
    assert!(result.violations.is_empty(),
        "Exact path match should work");
}
