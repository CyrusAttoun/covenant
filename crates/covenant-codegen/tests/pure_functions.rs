//! Integration tests for compiling pure functions to WASM
//!
//! Tests the full pipeline: parse -> check -> codegen -> execute

use wasmtime::{Engine, Instance, Linker, Module, Store};

/// Helper to compile source code to WASM and instantiate it
fn compile_and_instantiate(source: &str) -> (Store<()>, Instance) {
    // Parse
    let program = covenant_parser::parse(source)
        .expect("Failed to parse");

    // Type check
    let check_result = covenant_checker::check(&program)
        .expect("Type checking failed");

    // Compile to WASM
    let wasm_bytes = covenant_codegen::compile(&program, &check_result.symbols)
        .expect("WASM compilation failed");

    // Instantiate with wasmtime
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes)
        .expect("Failed to create WASM module");

    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);

    // Provide stub imports for all extern-abstract modules registered by codegen
    linker.func_wrap("mem", "alloc", |_size: i32| -> i32 { 0x10000 }).unwrap();

    // Provide no-op stubs for all other imported functions
    let module_ref = Module::new(&engine, &wasm_bytes).unwrap();
    for import in module_ref.imports() {
        let module_name = import.module();
        let name = import.name();
        if module_name == "mem" && name == "alloc" {
            continue;
        }
        match import.ty() {
            wasmtime::ExternType::Func(func_ty) => {
                let results_len = func_ty.results().len();
                if results_len == 0 {
                    let _ = linker.func_new(module_name, name, func_ty.clone(), |_caller, _params, _results| Ok(()));
                } else {
                    let _ = linker.func_new(module_name, name, func_ty.clone(), |_caller, _params, results| {
                        for r in results.iter_mut() {
                            *r = wasmtime::Val::I64(0);
                        }
                        Ok(())
                    });
                }
            }
            _ => {}
        }
    }

    let instance = linker.instantiate(&mut store, &module)
        .expect("Failed to instantiate module");

    (store, instance)
}

#[test]
fn test_compile_pure_add() {
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

    let (mut store, instance) = compile_and_instantiate(source);

    // Get the 'add' function
    let add = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "add")
        .expect("Failed to get 'add' function");

    // Test: add(2, 3) should return 5
    let result = add.call(&mut store, (2, 3)).expect("Call failed");
    assert_eq!(result, 5);

    // Test: add(10, -5) should return 5
    let result = add.call(&mut store, (10, -5)).expect("Call failed");
    assert_eq!(result, 5);

    // Test: add(0, 0) should return 0
    let result = add.call(&mut store, (0, 0)).expect("Call failed");
    assert_eq!(result, 0);
}

#[test]
fn test_compile_pure_double() {
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
"#;

    let (mut store, instance) = compile_and_instantiate(source);

    let double = instance
        .get_typed_func::<i64, i64>(&mut store, "double")
        .expect("Failed to get 'double' function");

    assert_eq!(double.call(&mut store, 5).unwrap(), 10);
    assert_eq!(double.call(&mut store, 0).unwrap(), 0);
    assert_eq!(double.call(&mut store, -3).unwrap(), -6);
}

#[test]
fn test_compile_pure_subtract() {
    let source = r#"
snippet id="math.subtract" kind="fn"

signature
  fn name="subtract"
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

    let (mut store, instance) = compile_and_instantiate(source);

    let subtract = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "subtract")
        .expect("Failed to get 'subtract' function");

    assert_eq!(subtract.call(&mut store, (10, 3)).unwrap(), 7);
    assert_eq!(subtract.call(&mut store, (5, 5)).unwrap(), 0);
    assert_eq!(subtract.call(&mut store, (3, 10)).unwrap(), -7);
}

#[test]
fn test_compile_multiple_functions() {
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


snippet id="math.mul" kind="fn"

signature
  fn name="mul"
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

    let (mut store, instance) = compile_and_instantiate(source);

    let add = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "add")
        .expect("Failed to get 'add' function");

    let mul = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "mul")
        .expect("Failed to get 'mul' function");

    assert_eq!(add.call(&mut store, (2, 3)).unwrap(), 5);
    assert_eq!(mul.call(&mut store, (2, 3)).unwrap(), 6);
}

#[test]
fn test_compile_simple_functions_from_example() {
    // Test compiling a subset of the pure-functions example
    // (factorial with recursion + conditionals is more complex - tested separately)
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
"#;

    let (mut store, instance) = compile_and_instantiate(source);

    // Test 'add' function
    let add = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "add")
        .expect("Failed to get 'add' function");
    assert_eq!(add.call(&mut store, (2, 3)).unwrap(), 5);

    // Test 'double' function
    let double = instance
        .get_typed_func::<i64, i64>(&mut store, "double")
        .expect("Failed to get 'double' function");
    assert_eq!(double.call(&mut store, 7).unwrap(), 14);
}

#[test]
fn test_compile_factorial() {
    // Test factorial with recursion and conditionals
    // This requires proper handling of if/else branches where both return
    let source = r#"
snippet id="math.factorial" kind="fn"

signature
  fn name="factorial"
    param name="n" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="compute"
    op=less_eq
    input var="n"
    input lit=1
    as="is_base"
  end
  step id="s2" kind="if"
    condition="is_base"
    then
      step id="s2a" kind="return"
        lit=1
        as="_"
      end
    end
    else
      step id="s2b" kind="compute"
        op=sub
        input var="n"
        input lit=1
        as="n_minus_1"
      end
      step id="s2c" kind="call"
        fn="factorial"
        arg name="n" from="n_minus_1"
        as="sub_result"
      end
      step id="s2d" kind="compute"
        op=mul
        input var="n"
        input var="sub_result"
        as="result"
      end
      step id="s2e" kind="return"
        from="result"
        as="_"
      end
    end
    as="_"
  end
end

end
"#;

    let (mut store, instance) = compile_and_instantiate(source);

    let factorial = instance
        .get_typed_func::<i64, i64>(&mut store, "factorial")
        .expect("Failed to get 'factorial' function");
    assert_eq!(factorial.call(&mut store, 0).unwrap(), 1);
    assert_eq!(factorial.call(&mut store, 1).unwrap(), 1);
    assert_eq!(factorial.call(&mut store, 5).unwrap(), 120);
}

// ============================================================================
// TDD Tests - Features not yet implemented
// These tests define expected behavior for future implementation.
// Tests in this section MUST FAIL until the feature is implemented.
// ============================================================================

// === Cross-Function Call Tests (FAILS: UndefinedFunction) ===

#[test]
fn test_compile_function_calls_function() {
    // TDD: Cross-function calls - currently fails with UndefinedFunction
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
    let (mut store, instance) = compile_and_instantiate(source);
    let quadruple = instance
        .get_typed_func::<i64, i64>(&mut store, "quadruple")
        .expect("Failed to get 'quadruple' function");
    assert_eq!(quadruple.call(&mut store, 5).unwrap(), 20);
    assert_eq!(quadruple.call(&mut store, 0).unwrap(), 0);
    assert_eq!(quadruple.call(&mut store, -3).unwrap(), -12);
}

// === Boolean Operation Tests ===

#[test]
fn test_compile_boolean_and() {
    let source = r#"
snippet id="logic.and" kind="fn"
signature
  fn name="and_fn"
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
    let (mut store, instance) = compile_and_instantiate(source);
    let and_fn = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "and_fn")
        .expect("Failed to get 'and_fn' function");
    assert_eq!(and_fn.call(&mut store, (1, 1)).unwrap(), 1);
    assert_eq!(and_fn.call(&mut store, (1, 0)).unwrap(), 0);
    assert_eq!(and_fn.call(&mut store, (0, 1)).unwrap(), 0);
    assert_eq!(and_fn.call(&mut store, (0, 0)).unwrap(), 0);
}

#[test]
fn test_compile_boolean_or() {
    let source = r#"
snippet id="logic.or" kind="fn"
signature
  fn name="or_fn"
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
    let (mut store, instance) = compile_and_instantiate(source);
    let or_fn = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "or_fn")
        .expect("Failed to get 'or_fn' function");
    assert_eq!(or_fn.call(&mut store, (1, 1)).unwrap(), 1);
    assert_eq!(or_fn.call(&mut store, (1, 0)).unwrap(), 1);
    assert_eq!(or_fn.call(&mut store, (0, 1)).unwrap(), 1);
    assert_eq!(or_fn.call(&mut store, (0, 0)).unwrap(), 0);
}

#[test]
fn test_compile_boolean_not() {
    let source = r#"
snippet id="logic.not" kind="fn"
signature
  fn name="not_fn"
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
    let (mut store, instance) = compile_and_instantiate(source);
    let not_fn = instance
        .get_typed_func::<i64, i64>(&mut store, "not_fn")
        .expect("Failed to get 'not_fn' function");
    assert_eq!(not_fn.call(&mut store, 1).unwrap(), 0);
    assert_eq!(not_fn.call(&mut store, 0).unwrap(), 1);
}

// === Comparison Operation Tests ===

#[test]
fn test_compile_comparison_equals() {
    let source = r#"
snippet id="cmp.eq" kind="fn"
signature
  fn name="eq"
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
    let (mut store, instance) = compile_and_instantiate(source);
    let eq = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "eq")
        .expect("Failed to get 'eq' function");
    assert_eq!(eq.call(&mut store, (5, 5)).unwrap(), 1);
    assert_eq!(eq.call(&mut store, (5, 3)).unwrap(), 0);
    assert_eq!(eq.call(&mut store, (0, 0)).unwrap(), 1);
}

#[test]
fn test_compile_comparison_less() {
    let source = r#"
snippet id="cmp.less" kind="fn"
signature
  fn name="less"
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
    let (mut store, instance) = compile_and_instantiate(source);
    let less = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "less")
        .expect("Failed to get 'less' function");
    assert_eq!(less.call(&mut store, (3, 5)).unwrap(), 1);
    assert_eq!(less.call(&mut store, (5, 3)).unwrap(), 0);
    assert_eq!(less.call(&mut store, (5, 5)).unwrap(), 0);
}

#[test]
fn test_compile_comparison_greater() {
    let source = r#"
snippet id="cmp.greater" kind="fn"
signature
  fn name="greater"
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
    let (mut store, instance) = compile_and_instantiate(source);
    let greater = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "greater")
        .expect("Failed to get 'greater' function");
    assert_eq!(greater.call(&mut store, (5, 3)).unwrap(), 1);
    assert_eq!(greater.call(&mut store, (3, 5)).unwrap(), 0);
    assert_eq!(greater.call(&mut store, (5, 5)).unwrap(), 0);
}

// === Additional Coverage Tests (these already pass) ===

#[test]
fn test_compile_nested_if() {
    // Tests nested if/else - already implemented
    let source = r#"
snippet id="math.clamp" kind="fn"
signature
  fn name="clamp"
    param name="x" type="Int"
    param name="min" type="Int"
    param name="max" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=less
    input var="x"
    input var="min"
    as="below_min"
  end
  step id="s2" kind="if"
    condition="below_min"
    then
      step id="s2a" kind="return"
        from="min"
        as="_"
      end
    end
    else
      step id="s2b" kind="compute"
        op=greater
        input var="x"
        input var="max"
        as="above_max"
      end
      step id="s2c" kind="if"
        condition="above_max"
        then
          step id="s2c1" kind="return"
            from="max"
            as="_"
          end
        end
        else
          step id="s2c2" kind="return"
            from="x"
            as="_"
          end
        end
        as="_"
      end
    end
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let clamp = instance
        .get_typed_func::<(i64, i64, i64), i64>(&mut store, "clamp")
        .expect("Failed to get 'clamp' function");
    assert_eq!(clamp.call(&mut store, (5, 0, 10)).unwrap(), 5);   // in range
    assert_eq!(clamp.call(&mut store, (-5, 0, 10)).unwrap(), 0);  // below min
    assert_eq!(clamp.call(&mut store, (15, 0, 10)).unwrap(), 10); // above max
}

#[test]
fn test_compile_bind_step() {
    // Tests bind step - already implemented
    let source = r#"
snippet id="test.bind" kind="fn"
signature
  fn name="test_bind"
    param name="x" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="bind"
    from="x"
    as="y"
  end
  step id="s2" kind="compute"
    op=add
    input var="y"
    input lit=10
    as="result"
  end
  step id="s3" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let test_bind = instance
        .get_typed_func::<i64, i64>(&mut store, "test_bind")
        .expect("Failed to get 'test_bind' function");
    assert_eq!(test_bind.call(&mut store, 5).unwrap(), 15);
    assert_eq!(test_bind.call(&mut store, 0).unwrap(), 10);
}

#[test]
fn test_compile_bind_literal() {
    // Tests bind with literal - already implemented
    let source = r#"
snippet id="test.const" kind="fn"
signature
  fn name="get_const"
    returns type="Int"
  end
end
body
  step id="s1" kind="bind"
    lit=42
    as="answer"
  end
  step id="s2" kind="return"
    from="answer"
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let get_const = instance
        .get_typed_func::<(), i64>(&mut store, "get_const")
        .expect("Failed to get 'get_const' function");
    assert_eq!(get_const.call(&mut store, ()).unwrap(), 42);
}

#[test]
fn test_compile_division() {
    // Tests division - already implemented
    let source = r#"
snippet id="math.div" kind="fn"
signature
  fn name="div"
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
    let (mut store, instance) = compile_and_instantiate(source);
    let div = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "div")
        .expect("Failed to get 'div' function");
    assert_eq!(div.call(&mut store, (10, 2)).unwrap(), 5);
    assert_eq!(div.call(&mut store, (7, 3)).unwrap(), 2);  // integer division
    assert_eq!(div.call(&mut store, (-10, 3)).unwrap(), -3); // or -4, depending on semantics
}

#[test]
fn test_compile_modulo() {
    // Tests modulo - already implemented
    let source = r#"
snippet id="math.mod" kind="fn"
signature
  fn name="modulo"
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
    let (mut store, instance) = compile_and_instantiate(source);
    let modulo = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "modulo")
        .expect("Failed to get 'modulo' function");
    assert_eq!(modulo.call(&mut store, (10, 3)).unwrap(), 1);
    assert_eq!(modulo.call(&mut store, (9, 3)).unwrap(), 0);
    assert_eq!(modulo.call(&mut store, (7, 4)).unwrap(), 3);
}

// ============================================================================
// More TDD Tests - These MUST FAIL until implemented
// ============================================================================

// === Match Step Tests (FAILS: match not implemented in codegen) ===

#[test]
fn test_compile_match_simple() {
    // TDD: Match expressions are not yet implemented in codegen
    // This tests matching on a simple enum-like discriminant
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

snippet id="test.check_status" kind="fn"
signature
  fn name="check_status"
    param name="status" type="Status"
    returns type="Int"
  end
end
body
  step id="s1" kind="match"
    on="status"
    case variant type="Status::Active"
      step id="c1" kind="return"
        lit=1
        as="_"
      end
    end
    case variant type="Status::Inactive"
      step id="c2" kind="return"
        lit=0
        as="_"
      end
    end
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let check = instance
        .get_typed_func::<i64, i64>(&mut store, "check_status")
        .expect("Failed to get 'check_status' function");
    assert_eq!(check.call(&mut store, 0).unwrap(), 1);  // Active
    assert_eq!(check.call(&mut store, 1).unwrap(), 0);  // Inactive
}

// === For Loop Tests ===

#[test]
fn test_compile_for_loop_sum() {
    // For loop that sums items from a list
    // List is passed as fat pointer: (ptr << 32) | length
    // We'll test with a simple list in memory
    let source = r#"
snippet id="math.sum_list" kind="fn"
signature
  fn name="sum_list"
    param name="items" type="List<Int>"
    returns type="Int"
  end
end
body
  step id="s1" kind="bind"
    lit=0
    as="total"
  end
  step id="s2" kind="for"
    var="item" in="items"
    step id="s2a" kind="compute"
      op=add
      input var="total"
      input var="item"
      as="total"
    end
    as="_"
  end
  step id="s3" kind="return"
    from="total"
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let sum_list = instance
        .get_typed_func::<i64, i64>(&mut store, "sum_list")
        .expect("Failed to get 'sum_list' function");

    // Get memory to write test arrays
    // For loops expect fat pointers: (ptr << 32) | length
    // Memory layout at ptr: [count:i32][item0:i64][item1:i64]...
    let memory = instance.get_memory(&mut store, "memory")
        .expect("Failed to get memory");

    // Helper: write array to memory at given offset, return fat pointer
    fn write_array(memory: &wasmtime::Memory, store: &mut wasmtime::Store<()>,
                   offset: u32, items: &[i64]) -> i64 {
        let data = memory.data_mut(store);
        // Write count (i32)
        data[offset as usize..offset as usize + 4]
            .copy_from_slice(&(items.len() as i32).to_le_bytes());
        // Write items (i64 each)
        for (i, item) in items.iter().enumerate() {
            let item_offset = offset as usize + 4 + i * 8;
            data[item_offset..item_offset + 8]
                .copy_from_slice(&item.to_le_bytes());
        }
        // Return fat pointer: (ptr << 32) | len
        ((offset as i64) << 32) | (items.len() as i64)
    }

    // Test 1: [0, 1, 2] -> sum = 3
    let ptr1 = write_array(&memory, &mut store, 1024, &[0, 1, 2]);
    assert_eq!(sum_list.call(&mut store, ptr1).unwrap(), 3);

    // Test 2: [0, 1, 2, 3, 4] -> sum = 10
    let ptr2 = write_array(&memory, &mut store, 2048, &[0, 1, 2, 3, 4]);
    assert_eq!(sum_list.call(&mut store, ptr2).unwrap(), 10);

    // Test 3: [0] -> sum = 0
    let ptr3 = write_array(&memory, &mut store, 3072, &[0]);
    assert_eq!(sum_list.call(&mut store, ptr3).unwrap(), 0);

    // Test 4: empty array -> sum = 0 (null pointer skips loop)
    let ptr4 = write_array(&memory, &mut store, 4096, &[]);
    assert_eq!(sum_list.call(&mut store, ptr4).unwrap(), 0);
}

// === String Type Tests (FAILS: String not implemented in WASM) ===

#[test]
fn test_compile_string_return() {
    // TDD: String handling not implemented in WASM codegen
    let source = r#"
snippet id="test.get_greeting" kind="fn"
signature
  fn name="get_greeting"
    returns type="String"
  end
end
body
  step id="s1" kind="return"
    lit="Hello, World!"
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let get_greeting = instance
        .get_typed_func::<(), i64>(&mut store, "get_greeting")
        .expect("Failed to get 'get_greeting' function");

    // String returned as fat pointer: (offset << 32) | length
    let result = get_greeting.call(&mut store, ()).unwrap();
    let offset = (result >> 32) as u32;
    let len = (result & 0xFFFFFFFF) as u32;

    // Verify the string encoding
    assert_eq!(offset, 0); // First string starts at offset 0
    assert_eq!(len, 13);   // "Hello, World!" is 13 characters

    // We can also read the actual string from memory
    let memory = instance.get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    let data = memory.data(&store);
    let string_bytes = &data[offset as usize..(offset + len) as usize];
    assert_eq!(std::str::from_utf8(string_bytes).unwrap(), "Hello, World!");
}

// === Struct Tests ===

#[test]
fn test_compile_struct_construction() {
    let source = r#"
snippet id="types.Point" kind="struct"
signature
  struct name="Point"
    field name="x" type="Int"
    field name="y" type="Int"
  end
end
end

snippet id="test.make_point" kind="fn"
signature
  fn name="make_point"
    param name="x" type="Int"
    param name="y" type="Int"
    returns type="Point"
  end
end
body
  step id="s1" kind="construct"
    type="Point"
    field name="x" from="x"
    field name="y" from="y"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let make_point = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "make_point")
        .expect("Failed to get 'make_point' function");

    // Result is a pointer (as i64) to memory where fields are stored
    let result = make_point.call(&mut store, (10, 20)).unwrap();
    let ptr = result as u32 as usize;

    // Read fields from memory: each field is i64 (8 bytes), field 0 at offset 0, field 1 at offset 8
    let memory = instance.get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    let data = memory.data(&store);
    let x = i64::from_le_bytes(data[ptr..ptr+8].try_into().unwrap());
    let y = i64::from_le_bytes(data[ptr+8..ptr+16].try_into().unwrap());
    assert_eq!(x, 10);
    assert_eq!(y, 20);

    // Test with different values
    let result2 = make_point.call(&mut store, (100, 200)).unwrap();
    let ptr2 = result2 as u32 as usize;
    let data2 = memory.data(&store);
    let x2 = i64::from_le_bytes(data2[ptr2..ptr2+8].try_into().unwrap());
    let y2 = i64::from_le_bytes(data2[ptr2+8..ptr2+16].try_into().unwrap());
    assert_eq!(x2, 100);
    assert_eq!(y2, 200);

    // Verify second allocation is after first (bump allocator)
    assert_eq!(ptr2, ptr + 16); // Point is 2 fields * 8 bytes = 16 bytes
}

#[test]
fn test_compile_struct_three_fields() {
    let source = r#"
snippet id="types.Vec3" kind="struct"
signature
  struct name="Vec3"
    field name="x" type="Int"
    field name="y" type="Int"
    field name="z" type="Int"
  end
end
end

snippet id="test.make_vec3" kind="fn"
signature
  fn name="make_vec3"
    param name="x" type="Int"
    param name="y" type="Int"
    param name="z" type="Int"
    returns type="Vec3"
  end
end
body
  step id="s1" kind="construct"
    type="Vec3"
    field name="x" from="x"
    field name="y" from="y"
    field name="z" from="z"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let make_vec3 = instance
        .get_typed_func::<(i64, i64, i64), i64>(&mut store, "make_vec3")
        .expect("Failed to get 'make_vec3' function");

    let result = make_vec3.call(&mut store, (1, 2, 3)).unwrap();
    let ptr = result as u32 as usize;

    let memory = instance.get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    let data = memory.data(&store);
    let x = i64::from_le_bytes(data[ptr..ptr+8].try_into().unwrap());
    let y = i64::from_le_bytes(data[ptr+8..ptr+16].try_into().unwrap());
    let z = i64::from_le_bytes(data[ptr+16..ptr+24].try_into().unwrap());
    assert_eq!(x, 1);
    assert_eq!(y, 2);
    assert_eq!(z, 3);
}

#[test]
fn test_compile_struct_field_access() {
    let source = r#"
snippet id="types.Point" kind="struct"
signature
  struct name="Point"
    field name="x" type="Int"
    field name="y" type="Int"
  end
end
end

snippet id="test.get_x" kind="fn"
signature
  fn name="get_x"
    param name="px" type="Int"
    param name="py" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="construct"
    type="Point"
    field name="x" from="px"
    field name="y" from="py"
    as="point"
  end
  step id="s2" kind="bind"
    field="x" of="point"
    as="result"
  end
  step id="s3" kind="return"
    from="result"
    as="_"
  end
end
end

snippet id="test.get_y" kind="fn"
signature
  fn name="get_y"
    param name="px" type="Int"
    param name="py" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="construct"
    type="Point"
    field name="x" from="px"
    field name="y" from="py"
    as="point"
  end
  step id="s2" kind="bind"
    field="y" of="point"
    as="result"
  end
  step id="s3" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);

    let get_x = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "get_x")
        .expect("Failed to get 'get_x' function");
    let get_y = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "get_y")
        .expect("Failed to get 'get_y' function");

    assert_eq!(get_x.call(&mut store, (42, 99)).unwrap(), 42);
    assert_eq!(get_y.call(&mut store, (42, 99)).unwrap(), 99);
    assert_eq!(get_x.call(&mut store, (0, -1)).unwrap(), 0);
    assert_eq!(get_y.call(&mut store, (0, -1)).unwrap(), -1);
}

#[test]
fn test_compile_struct_field_access_in_compute() {
    let source = r#"
snippet id="types.Point" kind="struct"
signature
  struct name="Point"
    field name="x" type="Int"
    field name="y" type="Int"
  end
end
end

snippet id="test.sum_fields" kind="fn"
signature
  fn name="sum_fields"
    param name="px" type="Int"
    param name="py" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="construct"
    type="Point"
    field name="x" from="px"
    field name="y" from="py"
    as="point"
  end
  step id="s2" kind="compute"
    op=add
    input field="x" of="point"
    input field="y" of="point"
    as="sum"
  end
  step id="s3" kind="return"
    from="sum"
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let sum_fields = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "sum_fields")
        .expect("Failed to get 'sum_fields' function");

    assert_eq!(sum_fields.call(&mut store, (10, 20)).unwrap(), 30);
    assert_eq!(sum_fields.call(&mut store, (100, 200)).unwrap(), 300);
    assert_eq!(sum_fields.call(&mut store, (-5, 5)).unwrap(), 0);
}

// === Optional Type Tests (FAILS: Optional handling not implemented) ===

#[test]
fn test_compile_optional_return() {
    // TDD: Optional/nullable values not implemented
    let source = r#"
snippet id="test.maybe_double" kind="fn"
signature
  fn name="maybe_double"
    param name="x" type="Int"
    param name="should_double" type="Bool"
    returns type="Int" optional
  end
end
body
  step id="s1" kind="if"
    condition="should_double"
    then
      step id="t1" kind="compute"
        op=mul
        input var="x"
        input lit=2
        as="doubled"
      end
      step id="t2" kind="return"
        from="doubled"
        as="_"
      end
    end
    else
      step id="e1" kind="return"
        lit=none
        as="_"
      end
    end
    as="_"
  end
end
end
"#;
    let (mut store, instance) = compile_and_instantiate(source);
    let maybe_double = instance
        .get_typed_func::<(i64, i64), i64>(&mut store, "maybe_double")
        .expect("Failed to get 'maybe_double' function");

    // When should_double is true (1), return doubled value
    assert_eq!(maybe_double.call(&mut store, (5, 1)).unwrap(), 10);
    assert_eq!(maybe_double.call(&mut store, (3, 1)).unwrap(), 6);

    // When should_double is false (0), return None (sentinel i64::MIN)
    assert_eq!(maybe_double.call(&mut store, (5, 0)).unwrap(), i64::MIN);
    assert_eq!(maybe_double.call(&mut store, (0, 0)).unwrap(), i64::MIN);
}
