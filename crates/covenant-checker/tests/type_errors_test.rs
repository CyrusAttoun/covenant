//! Integration tests for type error detection (Phase 4)

use covenant_checker::check;
use covenant_parser::parse;

/// Helper to check that parsing and type checking produces errors
fn check_source_has_errors(source: &str) -> Vec<covenant_checker::CheckError> {
    let program = parse(source).expect("parse failed");
    match check(&program) {
        Ok(_) => vec![],
        Err(errors) => errors,
    }
}

/// Helper to check that parsing and type checking succeeds
fn check_source_ok(source: &str) {
    let program = parse(source).expect("parse failed");
    let result = check(&program);
    assert!(result.is_ok(), "Expected check to succeed, got errors: {:?}", result.err());
}

// === Type Mismatch Tests ===

#[test]
fn test_return_type_mismatch() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit="not an int"
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    // Should have type mismatch error: returning String when Int expected
    assert!(!errors.is_empty(), "Expected type mismatch error");
}

#[test]
fn test_correct_return_type() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
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
    check_source_ok(source);
}

// === Undefined Variable Tests ===

#[test]
fn test_undefined_variable_in_return() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    from="undefined_var"
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Expected undefined variable error");
}

#[test]
fn test_undefined_variable_in_compute() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="x" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=add
    input var="x"
    input var="y"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Expected undefined variable 'y' error");
}

// === Compute Operation Type Tests ===

#[test]
fn test_compute_add_int_int() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
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
    check_source_ok(source);
}

#[test]
fn test_compute_equals_returns_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=equals
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
    check_source_ok(source);
}

// === If Statement Tests ===

#[test]
fn test_if_condition_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="x" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=less
    input var="x"
    input lit=0
    as="is_negative"
  end
  step id="s2" kind="if"
    condition="is_negative"
    then
      step id="s2a" kind="return"
        lit=0
        as="_"
      end
    end
    else
      step id="s2b" kind="return"
        from="x"
        as="_"
      end
    end
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Call Argument Type Tests ===

#[test]
fn test_call_correct_arg_type() {
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
    op=mul
    input var="x"
    input lit=2
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end

snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="n" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="math.double"
    arg name="x" from="n"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Multiple Snippets Interaction ===

#[test]
fn test_cross_snippet_call() {
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

snippet id="app.main" kind="fn"
signature
  fn name="main"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="math.add"
    arg name="a" lit=1
    arg name="b" lit=2
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Union Type Tests ===

#[test]
fn test_union_return_type() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="x" type="Int"
    returns union
      type="Int"
      type="String"
    end
  end
end
body
  step id="s1" kind="return"
    from="x"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Bind Step Tests ===

#[test]
fn test_bind_literal() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    returns type="Int"
  end
end
body
  step id="s1" kind="bind"
    lit=42
    as="x"
  end
  step id="s2" kind="return"
    from="x"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

#[test]
fn test_bind_from_param() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="input" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="bind"
    from="input"
    as="x"
  end
  step id="s2" kind="return"
    from="x"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// ==========================================================================
// COMPREHENSIVE PHASE 4 TESTS - Operators, Queries, Unions, Exhaustiveness
// ==========================================================================

// === All Operator Type Tests ===

#[test]
fn test_add_int_int_returns_int() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
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
    check_source_ok(source);
}

#[test]
fn test_sub_int_int_returns_int() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=sub
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
    check_source_ok(source);
}

#[test]
fn test_mul_int_int_returns_int() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=mul
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
    check_source_ok(source);
}

#[test]
fn test_div_int_int_returns_int() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=div
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
    check_source_ok(source);
}

#[test]
fn test_mod_int_int_returns_int() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=mod
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
    check_source_ok(source);
}

#[test]
fn test_less_returns_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=less
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
    check_source_ok(source);
}

#[test]
fn test_greater_returns_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=greater
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
    check_source_ok(source);
}

#[test]
fn test_less_eq_returns_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=less_eq
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
    check_source_ok(source);
}

#[test]
fn test_greater_eq_returns_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=greater_eq
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
    check_source_ok(source);
}

#[test]
fn test_and_requires_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Bool"
    param name="b" type="Bool"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=and
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
    check_source_ok(source);
}

#[test]
fn test_or_requires_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Bool"
    param name="b" type="Bool"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=or
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
    check_source_ok(source);
}

#[test]
fn test_not_requires_bool() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Bool"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=not
    input var="a"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Type Mismatch Error Tests ===

#[test]
fn test_and_with_int_is_error() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=and
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
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Should error when and is used with Int");
}

#[test]
fn test_or_with_int_is_error() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=or
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
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Should error when or is used with Int");
}

#[test]
fn test_not_with_int_is_error() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="a" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=not
    input var="a"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Should error when not is used with Int");
}

// === Function Call Type Tests ===

#[test]
#[ignore = "Cross-snippet argument type checking not yet implemented"]
fn test_call_wrong_arg_type() {
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

snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="math.add"
    arg name="a" lit="not an int"
    arg name="b" lit=2
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Should error when passing String to Int param");
}

#[test]
fn test_call_undefined_function() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="nonexistent.function"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Should error when calling undefined function");
}

// === If Condition Type Tests ===

#[test]
fn test_if_condition_not_bool_is_error() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="x" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="if"
    condition="x"
    then
      step id="s1a" kind="return"
        lit=1
        as="_"
      end
    end
    else
      step id="s1b" kind="return"
        lit=0
        as="_"
      end
    end
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Should error when if condition is Int, not Bool");
}

// === Optional Type Tests ===

#[test]
fn test_optional_return_none() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns type="Int" optional
  end
end
body
  step id="s1" kind="return"
    lit=none
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

#[test]
fn test_optional_return_value() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns type="Int" optional
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
    check_source_ok(source);
}

// === Union Type Tests ===

#[test]
fn test_union_return_first_member() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns union
      type="Int"
      type="String"
    end
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
    check_source_ok(source);
}

#[test]
fn test_union_return_second_member() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns union
      type="Int"
      type="String"
    end
  end
end
body
  step id="s1" kind="return"
    lit="hello"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

#[test]
#[ignore = "Union type member checking not yet implemented"]
fn test_union_return_non_member_is_error() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns union
      type="Int"
      type="String"
    end
  end
end
body
  step id="s1" kind="return"
    lit=true
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    assert!(!errors.is_empty(), "Should error when returning Bool for union of Int|String");
}

// === List Type Tests ===

#[test]
fn test_list_type_param() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="items" type="List<Int>"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit=0
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Struct Type Tests ===

#[test]
fn test_struct_field_types() {
    let source = r#"
snippet id="types.Point" kind="struct"
signature
  struct name="Point"
    field name="x" type="Int"
    field name="y" type="Int"
  end
end
end

snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="p" type="Point"
    returns type="Int"
  end
end
body
  step id="s1" kind="return"
    lit=0
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Enum Type Tests ===

#[test]
fn test_enum_definition() {
    let source = r#"
snippet id="types.Status" kind="enum"
signature
  enum name="Status"
    variant name="Active"
    end
    variant name="Inactive"
    end
  end
end
end

snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="s" type="Status"
    returns type="Bool"
  end
end
body
  step id="s1" kind="return"
    lit=true
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Chain of Operations ===

#[test]
fn test_chain_of_operations() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
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
    as="product"
  end
  step id="s3" kind="compute"
    op=sub
    input var="product"
    input lit=1
    as="result"
  end
  step id="s4" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Multiple Errors ===

#[test]
#[ignore = "Multiple type error collection not yet implemented"]
fn test_multiple_type_errors() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=and
    input lit=1
    input lit=2
    as="bad1"
  end
  step id="s2" kind="compute"
    op=add
    input var="undefined_var"
    input lit=3
    as="bad2"
  end
  step id="s3" kind="return"
    lit="wrong type"
    as="_"
  end
end
end
"#;
    let errors = check_source_has_errors(source);
    // Should have multiple errors
    assert!(errors.len() >= 2, "Expected multiple errors, got: {:?}", errors);
}

// === Extern Functions ===

#[test]
#[ignore = "Extern function call resolution not yet implemented"]
fn test_extern_function_signature() {
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
    fn="io.print"
    arg name="msg" lit="Hello"
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}

// === Complex Nested Structures ===

#[test]
fn test_nested_if_type_consistency() {
    let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    param name="x" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=greater
    input var="x"
    input lit=0
    as="positive"
  end
  step id="s2" kind="if"
    condition="positive"
    then
      step id="s2a" kind="compute"
        op=greater
        input var="x"
        input lit=10
        as="big"
      end
      step id="s2b" kind="if"
        condition="big"
        then
          step id="s2b1" kind="return"
            lit=100
            as="_"
          end
        end
        else
          step id="s2b2" kind="return"
            lit=10
            as="_"
          end
        end
        as="_"
      end
    end
    else
      step id="s2c" kind="return"
        lit=0
        as="_"
      end
    end
    as="_"
  end
end
end
"#;
    check_source_ok(source);
}
