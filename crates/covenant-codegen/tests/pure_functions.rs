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
    let linker = Linker::new(&engine);
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
