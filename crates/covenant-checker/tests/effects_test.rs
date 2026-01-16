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
