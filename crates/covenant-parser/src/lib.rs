//! Covenant Parser - Recursive descent parser
//!
//! Parses Covenant source code into an AST.
//! Key parsing challenges:
//! - No `fn` keyword: functions are identified by signature shape
//! - `=` is equality, `:=` is assignment
//! - Query expressions with SQL-like syntax

mod error;
mod parser;

pub use error::*;
pub use parser::*;

use covenant_ast::Program;
use covenant_lexer::tokenize;

/// Parse a source string into a Program AST
pub fn parse(source: &str) -> Result<Program, ParseError> {
    let tokens = tokenize(source);
    let mut parser = Parser::new(source, tokens);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use covenant_ast::Section;

    #[test]
    fn test_parse_hello_world() {
        let source = r#"
            main()
                import { println } from console
            {
                println("Hello, world!")
            }
        "#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_struct() {
        let source = r#"
            struct User {
                id: Int,
                name: String,
            }
        "#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_pure_function() {
        let source = r#"
            double(x: Int) -> Int {
                x * 2
            }
        "#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    // === Snippet Kind Tests ===

    #[test]
    fn test_parse_snippet_fn() {
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
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse fn snippet: {:?}", result.err());
        let program = result.unwrap();
        if let Program::Snippets { snippets, .. } = program {
            assert_eq!(snippets.len(), 1);
            assert_eq!(snippets[0].id, "math.add");
        } else {
            panic!("Expected Snippets program");
        }
    }

    #[test]
    fn test_parse_extern_snippet() {
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

metadata
  contract="console.log@1"
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse extern snippet: {:?}", result.err());
    }

    #[test]
    fn test_parse_struct_snippet() {
        let source = r#"
snippet id="types.User" kind="struct"

signature
  struct name="User"
    field name="id" type="Int"
    field name="name" type="String"
    field name="email" type="String" optional
  end
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse struct snippet: {:?}", result.err());
    }

    #[test]
    fn test_parse_enum_snippet() {
        let source = r#"
snippet id="types.Result" kind="enum"

signature
  enum name="Result"
    variant name="Ok"
      field name="value" type="Int"
    end
    variant name="Err"
      field name="message" type="String"
    end
  end
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse enum snippet: {:?}", result.err());
    }

    // === Step Kind Tests ===

    #[test]
    fn test_parse_compute_step() {
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
    input lit=1
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse compute step: {:?}", result.err());
    }

    #[test]
    fn test_parse_call_step() {
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="x" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="call"
    fn="math.double"
    arg name="x" from="x"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse call step: {:?}", result.err());
    }

    #[test]
    fn test_parse_if_step() {
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
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse if step: {:?}", result.err());
    }

    #[test]
    fn test_parse_match_step() {
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="value" type="Result"
    returns type="Int"
  end
end
body
  step id="s1" kind="match"
    on="value"
    case variant type="Result::Ok" bindings=("v")
      step id="s1a" kind="return"
        from="v"
        as="_"
      end
    end
    case wildcard
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
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse match step: {:?}", result.err());
    }

    #[test]
    fn test_parse_for_step() {
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="items" type="List<Int>"
    returns type="List<Int>"
  end
end
body
  step id="s1" kind="call"
    fn="new_list"
    as="result"
  end
  step id="s2" kind="for"
    var="item" in="items"
    step id="s2a" kind="call"
      fn="push"
      arg name="list" from="result"
      arg name="item" from="item"
      as="result"
    end
    as="_"
  end
  step id="s3" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse for step: {:?}", result.err());
    }

    // === Query Tests ===

    #[test]
    fn test_parse_query_covenant_dialect() {
        let source = r#"
snippet id="test.fn" kind="fn"
effects
  effect database
end
signature
  fn name="test_fn"
    returns type="List<User>"
  end
end
body
  step id="s1" kind="query"
    target="project"
    select all
    from="users"
    where
      equals field="active" lit=true
    end
    order by="name" dir="asc"
    limit=10
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse Covenant query: {:?}", result.err());
    }

    #[test]
    fn test_parse_query_sql_dialect() {
        let source = r#"
snippet id="test.fn" kind="fn"
effects
  effect database
end
signature
  fn name="test_fn"
    param name="user_id" type="Int"
    returns type="List<Order>"
  end
end
body
  step id="s1" kind="query"
    dialect="postgres"
    target="app_db"
    body
      SELECT * FROM orders WHERE user_id = :user_id
    end
    params
      param name="user_id" from="user_id"
    end
    returns collection of="Order"
    as="orders"
  end
  step id="s2" kind="return"
    from="orders"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse SQL dialect query: {:?}", result.err());
    }

    // === Type Syntax Tests ===

    #[test]
    fn test_parse_optional_type() {
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="value" type="Json"
    returns type="String" optional
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
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse optional type: {:?}", result.err());
    }

    #[test]
    fn test_parse_list_type() {
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    param name="items" type="List<Int>"
    returns type="List<String>"
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
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse List type: {:?}", result.err());
    }

    #[test]
    fn test_parse_union_type() {
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test_fn"
    returns union
      type="Int"
      type="String"
      type="Error"
    end
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
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse union type: {:?}", result.err());
    }

    // === Section Tests ===

    #[test]
    fn test_parse_snippet_with_all_sections() {
        let source = r#"
snippet id="test.fn" kind="fn"

effects
  effect database
  effect network
end

requires
  req id="R-001"
    text "Must handle null input"
    priority high
  end
end

signature
  fn name="test_fn"
    param name="x" type="Int"
    returns type="Int"
  end
end

body
  step id="s1" kind="return"
    from="x"
    as="_"
  end
end

tests
  test id="T-001" kind="unit" covers="R-001"
  end
end

metadata
  author="test"
  version="1.0"
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse snippet with all sections: {:?}", result.err());
    }

    // === Multiple Snippets ===

    #[test]
    fn test_parse_multiple_snippets() {
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

snippet id="math.sub" kind="fn"
signature
  fn name="sub"
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
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse multiple snippets: {:?}", result.err());
        let program = result.unwrap();
        if let Program::Snippets { snippets, .. } = program {
            assert_eq!(snippets.len(), 2);
        } else {
            panic!("Expected Snippets program");
        }
    }

    // === Error Cases ===

    #[test]
    fn test_parse_empty_source() {
        let result = parse("");
        assert!(result.is_ok(), "Empty source should parse successfully");
        let program = result.unwrap();
        match program {
            Program::Snippets { snippets, .. } => assert!(snippets.is_empty()),
            Program::Legacy { declarations, .. } => assert!(declarations.is_empty()),
        }
    }

    #[test]
    fn test_parse_comments_only() {
        let source = r#"
// This is a comment
// Another comment
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Comments-only should parse successfully");
    }

    #[test]
    fn test_parse_unclosed_snippet() {
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns type="Int"
  end
end
body
end
"#;
        // Missing final "end" for snippet
        let result = parse(source);
        assert!(result.is_err(), "Unclosed snippet should fail to parse");
    }

    #[test]
    fn test_parse_missing_snippet_id() {
        let source = r#"
snippet kind="fn"
signature
  fn name="test"
    returns type="Int"
  end
end
body
end
end
"#;
        let result = parse(source);
        assert!(result.is_err(), "Missing snippet id should fail to parse");
    }

    // ==========================================================================
    // COMPREHENSIVE PHASE 1 TESTS - SQL Dialects, Transactions, Traverse, etc.
    // ==========================================================================

    // === SQL Dialect Tests ===

    #[test]
    fn test_parse_query_sql_postgres_dialect() {
        let source = r#"
snippet id="db.get_users" kind="fn"
effects
  effect database
end
signature
  fn name="get_users"
    param name="active" type="Bool"
    returns collection of="User"
  end
end
body
  step id="s1" kind="query"
    dialect="postgres"
    target="app_db"
    body
      SELECT id, name, email FROM users WHERE active = :active ORDER BY created_at DESC
    end
    params
      param name="active" from="active"
    end
    returns collection of="User"
    as="users"
  end
  step id="s2" kind="return"
    from="users"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse postgres SQL dialect: {:?}", result.err());
    }

    #[test]
    fn test_parse_query_sql_sqlserver_dialect() {
        let source = r#"
snippet id="db.get_orders" kind="fn"
effects
  effect database
end
signature
  fn name="get_orders"
    param name="customer_id" type="Int"
    returns collection of="Order"
  end
end
body
  step id="s1" kind="query"
    dialect="sqlserver"
    target="orders_db"
    body
      SELECT TOP 100 id, total, status FROM orders WHERE customer_id = @customer_id ORDER BY order_date DESC
    end
    params
      param name="customer_id" from="customer_id"
    end
    returns collection of="Order"
    as="orders"
  end
  step id="s2" kind="return"
    from="orders"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse sqlserver SQL dialect: {:?}", result.err());
    }

    #[test]
    fn test_parse_query_sql_mysql_dialect() {
        let source = r#"
snippet id="db.count_products" kind="fn"
effects
  effect database
end
signature
  fn name="count_products"
    param name="category" type="String"
    returns type="Int"
  end
end
body
  step id="s1" kind="query"
    dialect="mysql"
    target="inventory_db"
    body
      SELECT COUNT(*) as count FROM products WHERE category = ?
    end
    params
      param name="category" from="category"
    end
    returns type="Int"
    as="count"
  end
  step id="s2" kind="return"
    from="count"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse mysql SQL dialect: {:?}", result.err());
    }

    #[test]
    fn test_parse_query_sql_sqlite_dialect() {
        let source = r#"
snippet id="db.get_config" kind="fn"
effects
  effect database
end
signature
  fn name="get_config"
    param name="key" type="String"
    returns type="String" optional
  end
end
body
  step id="s1" kind="query"
    dialect="sqlite"
    target="config_db"
    body
      SELECT value FROM config WHERE key = :key LIMIT 1
    end
    params
      param name="key" from="key"
    end
    returns type="String" optional
    as="value"
  end
  step id="s2" kind="return"
    from="value"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse sqlite SQL dialect: {:?}", result.err());
    }

    #[test]
    fn test_parse_query_sql_complex_with_cte() {
        let source = r#"
snippet id="db.analytics" kind="fn"
effects
  effect database
end
signature
  fn name="get_user_stats"
    param name="min_orders" type="Int"
    returns collection of="UserStats"
  end
end
body
  step id="s1" kind="query"
    dialect="postgres"
    target="analytics_db"
    body
      WITH order_counts AS (
        SELECT user_id, COUNT(*) as order_count
        FROM orders
        GROUP BY user_id
      )
      SELECT u.id, u.name, oc.order_count
      FROM users u
      JOIN order_counts oc ON oc.user_id = u.id
      WHERE oc.order_count >= :min_orders
    end
    params
      param name="min_orders" from="min_orders"
    end
    returns collection of="UserStats"
    as="stats"
  end
  step id="s2" kind="return"
    from="stats"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse complex SQL with CTE: {:?}", result.err());
    }

    // === Transaction Step Tests ===

    #[test]
    #[ignore = "Transaction step parsing not yet implemented"]
    fn test_parse_transaction_step_basic() {
        let source = r#"
snippet id="db.transfer" kind="fn"
effects
  effect database
end
signature
  fn name="transfer"
    param name="from_account" type="Int"
    param name="to_account" type="Int"
    param name="amount" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="transaction"
    target="bank_db"
    step id="t1" kind="query"
      dialect="postgres"
      body
        UPDATE accounts SET balance = balance - :amount WHERE id = :from_account
      end
      params
        param name="amount" from="amount"
        param name="from_account" from="from_account"
      end
      as="_"
    end
    step id="t2" kind="query"
      dialect="postgres"
      body
        UPDATE accounts SET balance = balance + :amount WHERE id = :to_account
      end
      params
        param name="amount" from="amount"
        param name="to_account" from="to_account"
      end
      as="_"
    end
    as="tx_result"
  end
  step id="s2" kind="return"
    lit=true
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse transaction step: {:?}", result.err());
    }

    #[test]
    #[ignore = "Transaction step parsing not yet implemented"]
    fn test_parse_transaction_with_isolation_level() {
        let source = r#"
snippet id="db.critical_update" kind="fn"
effects
  effect database
end
signature
  fn name="critical_update"
    param name="id" type="Int"
    param name="value" type="String"
    returns type="Bool"
  end
end
body
  step id="s1" kind="transaction"
    target="main_db"
    isolation="serializable"
    step id="t1" kind="query"
      dialect="postgres"
      body
        UPDATE critical_data SET value = :value WHERE id = :id
      end
      params
        param name="value" from="value"
        param name="id" from="id"
      end
      as="_"
    end
    as="tx"
  end
  step id="s2" kind="return"
    lit=true
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse transaction with isolation level: {:?}", result.err());
    }

    // === Traverse Step Tests ===

    #[test]
    fn test_parse_traverse_step_basic() {
        let source = r#"
snippet id="graph.find_deps" kind="fn"
signature
  fn name="find_dependencies"
    param name="module_id" type="String"
    returns collection of="Module"
  end
end
body
  step id="s1" kind="traverse"
    target="project"
    from="module_id"
    follow type="depends_on"
    depth=3
    as="deps"
  end
  step id="s2" kind="return"
    from="deps"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse traverse step: {:?}", result.err());
    }

    #[test]
    fn test_parse_traverse_with_direction() {
        let source = r#"
snippet id="graph.find_callers" kind="fn"
signature
  fn name="find_callers"
    param name="fn_id" type="String"
    returns collection of="Function"
  end
end
body
  step id="s1" kind="traverse"
    target="project"
    from="fn_id"
    follow type="calls" direction="inbound"
    as="callers"
  end
  step id="s2" kind="return"
    from="callers"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse traverse with direction: {:?}", result.err());
    }

    // === Construct Step Tests ===

    #[test]
    fn test_parse_construct_struct() {
        let source = r#"
snippet id="types.Point" kind="struct"
signature
  struct name="Point"
    field name="x" type="Int"
    field name="y" type="Int"
  end
end
end

snippet id="geo.make_point" kind="fn"
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
    as="point"
  end
  step id="s2" kind="return"
    from="point"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse construct step: {:?}", result.err());
    }

    #[test]
    fn test_parse_construct_with_literal_fields() {
        let source = r#"
snippet id="types.Config" kind="struct"
signature
  struct name="Config"
    field name="debug" type="Bool"
    field name="timeout" type="Int"
  end
end
end

snippet id="config.default" kind="fn"
signature
  fn name="default_config"
    returns type="Config"
  end
end
body
  step id="s1" kind="construct"
    type="Config"
    field name="debug" lit=false
    field name="timeout" lit=30
    as="config"
  end
  step id="s2" kind="return"
    from="config"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse construct with literal fields: {:?}", result.err());
    }

    // === Bind Step Tests ===

    #[test]
    fn test_parse_bind_from_var() {
        let source = r#"
snippet id="test.alias" kind="fn"
signature
  fn name="alias"
    param name="x" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="bind"
    from="x"
    as="y"
  end
  step id="s2" kind="return"
    from="y"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse bind from var: {:?}", result.err());
    }

    #[test]
    fn test_parse_bind_literal_string() {
        let source = r#"
snippet id="test.const_str" kind="fn"
signature
  fn name="get_greeting"
    returns type="String"
  end
end
body
  step id="s1" kind="bind"
    lit="Hello, World!"
    as="greeting"
  end
  step id="s2" kind="return"
    from="greeting"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse bind literal string: {:?}", result.err());
    }

    #[test]
    fn test_parse_bind_literal_bool() {
        let source = r#"
snippet id="test.const_bool" kind="fn"
signature
  fn name="get_flag"
    returns type="Bool"
  end
end
body
  step id="s1" kind="bind"
    lit=true
    as="flag"
  end
  step id="s2" kind="return"
    from="flag"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse bind literal bool: {:?}", result.err());
    }

    // === CRUD Operation Tests ===

    #[test]
    fn test_parse_insert_step() {
        let source = r#"
snippet id="db.create_user" kind="fn"
effects
  effect database
end
signature
  fn name="create_user"
    param name="name" type="String"
    param name="email" type="String"
    returns type="User"
  end
end
body
  step id="s1" kind="insert"
    into="project.users"
    set field="name" from="name"
    set field="email" from="email"
    as="new_user"
  end
  step id="s2" kind="return"
    from="new_user"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse insert step: {:?}", result.err());
    }

    #[test]
    fn test_parse_update_step() {
        let source = r#"
snippet id="db.update_user" kind="fn"
effects
  effect database
end
signature
  fn name="update_user"
    param name="user_id" type="Int"
    param name="new_name" type="String"
    returns type="User"
  end
end
body
  step id="s1" kind="update"
    target="project.users"
    set field="name" from="new_name"
    where
      equals field="id" var="user_id"
    end
    as="updated_user"
  end
  step id="s2" kind="return"
    from="updated_user"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse update step: {:?}", result.err());
    }

    #[test]
    fn test_parse_delete_step() {
        let source = r#"
snippet id="db.delete_user" kind="fn"
effects
  effect database
end
signature
  fn name="delete_user"
    param name="user_id" type="Int"
    returns type="Bool"
  end
end
body
  step id="s1" kind="delete"
    from="project.users"
    where
      equals field="id" var="user_id"
    end
    as="deleted"
  end
  step id="s2" kind="return"
    lit=true
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse delete step: {:?}", result.err());
    }

    // === All Compute Operations ===

    #[test]
    fn test_parse_compute_all_binary_ops() {
        // Test all binary operators parse correctly
        let ops = vec![
            ("add", "+"),
            ("sub", "-"),
            ("mul", "*"),
            ("div", "/"),
            ("mod", "%"),
            ("equals", "=="),
            ("not_equals", "!="),
            ("less", "<"),
            ("less_eq", "<="),
            ("greater", ">"),
            ("greater_eq", ">="),
            ("and", "&&"),
            ("or", "||"),
        ];

        for (op_name, _symbol) in ops {
            let source = format!(r#"
snippet id="test.{op_name}" kind="fn"
signature
  fn name="{op_name}"
    param name="a" type="Int"
    param name="b" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op={op_name}
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
"#, op_name = op_name);
            let result = parse(&source);
            assert!(result.is_ok(), "Failed to parse compute op={}: {:?}", op_name, result.err());
        }
    }

    #[test]
    fn test_parse_compute_unary_ops() {
        let source = r#"
snippet id="test.negate" kind="fn"
signature
  fn name="negate"
    param name="x" type="Bool"
    returns type="Bool"
  end
end
body
  step id="s1" kind="compute"
    op=not
    input var="x"
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse unary not: {:?}", result.err());
    }

    // === Concurrent Steps ===

    #[test]
    fn test_parse_parallel_step() {
        let source = r#"
snippet id="app.fetch_all" kind="fn"
effects
  effect network
end
signature
  fn name="fetch_all"
    returns type="Results"
  end
end
body
  step id="s1" kind="parallel"
    branch id="b1"
      step id="b1.1" kind="call"
        fn="http.get"
        arg name="url" lit="https://api.example.com/users"
        as="users"
      end
    end
    branch id="b2"
      step id="b2.1" kind="call"
        fn="http.get"
        arg name="url" lit="https://api.example.com/products"
        as="products"
      end
    end
    as="results"
  end
  step id="s2" kind="return"
    from="results"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse parallel step: {:?}", result.err());
    }

    #[test]
    fn test_parse_race_step() {
        let source = r#"
snippet id="cache.get_with_fallback" kind="fn"
effects
  effect network
end
signature
  fn name="get_with_fallback"
    param name="key" type="String"
    returns type="String"
  end
end
body
  step id="s1" kind="race"
    branch id="b1"
      step id="b1.1" kind="call"
        fn="cache.get"
        arg name="key" from="key"
        as="cached"
      end
    end
    branch id="b2"
      step id="b2.1" kind="call"
        fn="api.fetch"
        arg name="key" from="key"
        as="fetched"
      end
    end
    as="result"
  end
  step id="s2" kind="return"
    from="result"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse race step: {:?}", result.err());
    }

    // === Database Binding Tests ===

    #[test]
    #[ignore = "Database snippet kind parsing not yet implemented"]
    fn test_parse_database_snippet() {
        let source = r#"
snippet id="db.main_db" kind="database"

metadata
  type="database"
  dialect="postgres"
  connection="env:DATABASE_URL"
end

schema
  table name="users"
    field name="id" type="Int" primary_key=true
    field name="name" type="String"
    field name="email" type="String"
    field name="created_at" type="DateTime"
  end
  table name="orders"
    field name="id" type="Int" primary_key=true
    field name="user_id" type="Int" foreign_key="users.id"
    field name="total" type="Int"
  end
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse database snippet: {:?}", result.err());
    }

    // === Data Snippet Tests ===

    #[test]
    fn test_parse_data_snippet() {
        let source = r#"
snippet id="docs.api_design" kind="data"

metadata
  type="documentation"
  format="markdown"
end

content
  """
  # API Design Guidelines

  This document describes our API design patterns.

  ## Versioning
  All APIs should be versioned with /v1/, /v2/, etc.
  """
end

relations
  rel to="docs.rest_conventions" type="references"
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse data snippet: {:?}", result.err());
    }

    // === Error Recovery Tests ===

    #[test]
    fn test_parse_recovers_from_bad_step_kind() {
        // Parser should ideally report error but continue parsing
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns type="Int"
  end
end
body
  step id="s1" kind="invalid_kind"
    as="x"
  end
end
end
"#;
        let result = parse(source);
        // Should fail but with clear error about invalid step kind
        assert!(result.is_err(), "Invalid step kind should produce error");
        let err = result.unwrap_err();
        // Error message should mention the invalid kind
        let err_str = format!("{:?}", err);
        assert!(
            err_str.contains("invalid") || err_str.contains("unexpected") || err_str.contains("kind"),
            "Error should mention invalid kind: {}", err_str
        );
    }

    #[test]
    fn test_parse_error_has_span_info() {
        let source = r#"
snippet id="test.fn" kind="fn"
signature
  fn name="test"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=add
    input var="a"
    // Missing second input and as clause
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_err(), "Malformed step should produce error");
        let err = result.unwrap_err();
        // Error should have span information
        assert!(err.span().start > 0 || err.span().end > 0, "Error should have span info");
    }

    // === Edge Cases ===

    #[test]
    fn test_parse_empty_body() {
        let source = r#"
snippet id="test.empty" kind="fn"
signature
  fn name="empty"
    returns type="Unit"
  end
end
body
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Empty body should be valid: {:?}", result.err());
    }

    #[test]
    fn test_parse_deeply_nested_if() {
        let source = r#"
snippet id="test.nested" kind="fn"
signature
  fn name="nested"
    param name="x" type="Int"
    returns type="Int"
  end
end
body
  step id="s1" kind="compute"
    op=greater
    input var="x"
    input lit=0
    as="c1"
  end
  step id="s2" kind="if"
    condition="c1"
    then
      step id="s2a" kind="compute"
        op=greater
        input var="x"
        input lit=10
        as="c2"
      end
      step id="s2b" kind="if"
        condition="c2"
        then
          step id="s2b1" kind="compute"
            op=greater
            input var="x"
            input lit=100
            as="c3"
          end
          step id="s2b2" kind="if"
            condition="c3"
            then
              step id="deep" kind="return"
                lit=1000
                as="_"
              end
            end
            else
              step id="deep2" kind="return"
                lit=100
                as="_"
              end
            end
            as="_"
          end
        end
        else
          step id="s2b3" kind="return"
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
        let result = parse(source);
        assert!(result.is_ok(), "Deeply nested if should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_string_with_escapes() {
        let source = r#"
snippet id="test.escapes" kind="fn"
signature
  fn name="get_escaped"
    returns type="String"
  end
end
body
  step id="s1" kind="bind"
    lit="Hello\nWorld\t\"Escaped\""
    as="text"
  end
  step id="s2" kind="return"
    from="text"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "String with escapes should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_triple_quoted_string() {
        let source = r#"
snippet id="test.multiline" kind="fn"
signature
  fn name="get_multiline"
    returns type="String"
  end
end
body
  step id="s1" kind="bind"
    lit="""
    This is a
    multi-line
    string
    """
    as="text"
  end
  step id="s2" kind="return"
    from="text"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Triple quoted string should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_generic_types() {
        let source = r#"
snippet id="test.generic" kind="fn"
signature
  fn name="process"
    param name="items" type="List<Map<String, Int>>"
    returns type="Set<String>"
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
        let result = parse(source);
        assert!(result.is_ok(), "Nested generic types should parse: {:?}", result.err());
    }

    #[test]
    #[ignore = "Return with variant syntax not yet implemented"]
    fn test_parse_return_with_variant() {
        let source = r#"
snippet id="test.variant" kind="fn"
signature
  fn name="get_result"
    returns union
      type="Int"
      type="Error"
    end
  end
end
body
  step id="s1" kind="return"
    variant type="Error"
    field name="message" lit="Something went wrong"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Return with variant should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_none_literal() {
        let source = r#"
snippet id="test.none" kind="fn"
signature
  fn name="get_nothing"
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
        let result = parse(source);
        assert!(result.is_ok(), "None literal should parse: {:?}", result.err());
    }

    #[test]
    #[ignore = "Covenant query with join syntax not yet implemented"]
    fn test_parse_query_with_join() {
        let source = r#"
snippet id="db.users_with_orders" kind="fn"
effects
  effect database
end
signature
  fn name="get_users_with_orders"
    returns collection of="UserOrder"
  end
end
body
  step id="s1" kind="query"
    target="project"
    select all
    from="users"
    join target="orders" on="orders.user_id" equals="users.id"
    as="results"
  end
  step id="s2" kind="return"
    from="results"
    as="_"
  end
end
end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Query with join should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_requirements_with_all_priorities() {
        let source = r#"
snippet id="test.fn" kind="fn"

requires
  req id="R-001"
    text "Critical requirement"
    priority critical
  end
  req id="R-002"
    text "High priority requirement"
    priority high
  end
  req id="R-003"
    text "Medium priority requirement"
    priority medium
  end
  req id="R-004"
    text "Low priority requirement"
    priority low
  end
end

signature
  fn name="test"
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
        let result = parse(source);
        assert!(result.is_ok(), "Requirements with all priorities should parse: {:?}", result.err());
    }

    #[test]
    #[ignore = "Test section with assert step kind not yet implemented"]
    fn test_parse_test_section() {
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

tests
  test id="T-001" kind="unit" covers="R-001"
    step id="t1" kind="call"
      fn="math.add"
      arg name="a" lit=2
      arg name="b" lit=3
      as="result"
    end
    step id="t2" kind="assert"
      op=equals
      input var="result"
      input lit=5
      as="_"
    end
  end
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Test section with steps should parse: {:?}", result.err());
    }

    #[test]
    fn test_parse_dotted_effect_name() {
        let source = r#"
snippet id="storage.save" kind="fn"

effects
  effect std.storage
end

signature
  fn name="save"
    param name="key" type="String"
    param name="value" type="String"
    returns type="Unit"
  end
end

body
  step id="s1" kind="call"
    fn="std.storage.kv.set"
    arg name="key" from="key"
    arg name="value" from="value"
    as="_"
  end
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse dotted effect name: {:?}", result.err());
        let program = result.unwrap();
        if let Program::Snippets { snippets, .. } = program {
            let snippet = &snippets[0];
            let effects_section = snippet.sections.iter().find_map(|s| {
                if let Section::Effects(e) = s { Some(e) } else { None }
            }).expect("effects section not found");
            assert_eq!(effects_section.effects.len(), 1);
            assert_eq!(effects_section.effects[0].name, "std.storage");
        } else {
            panic!("Expected Snippets program");
        }
    }

    #[test]
    fn test_parse_multi_level_dotted_effect() {
        let source = r#"
snippet id="db.reader" kind="fn"

effects
  effect database.postgres.read
end

signature
  fn name="read_users"
    returns type="String"
  end
end

body
  step id="s1" kind="return"
    lit="placeholder"
    as="_"
  end
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse multi-level dotted effect: {:?}", result.err());
        let program = result.unwrap();
        if let Program::Snippets { snippets, .. } = program {
            let snippet = &snippets[0];
            let effects_section = snippet.sections.iter().find_map(|s| {
                if let Section::Effects(e) = s { Some(e) } else { None }
            }).expect("effects section not found");
            assert_eq!(effects_section.effects[0].name, "database.postgres.read");
        } else {
            panic!("Expected Snippets program");
        }
    }

    #[test]
    fn test_parse_multiple_dotted_effects() {
        let source = r#"
snippet id="api.handler" kind="fn"

effects
  effect network.http
  effect database.read
  effect console
end

signature
  fn name="handle"
    returns type="Unit"
  end
end

body
  step id="s1" kind="return"
    lit="done"
    as="_"
  end
end

end
"#;
        let result = parse(source);
        assert!(result.is_ok(), "Failed to parse multiple dotted effects: {:?}", result.err());
        let program = result.unwrap();
        if let Program::Snippets { snippets, .. } = program {
            let snippet = &snippets[0];
            let effects_section = snippet.sections.iter().find_map(|s| {
                if let Section::Effects(e) = s { Some(e) } else { None }
            }).expect("effects section not found");
            assert_eq!(effects_section.effects.len(), 3);
            assert_eq!(effects_section.effects[0].name, "network.http");
            assert_eq!(effects_section.effects[1].name, "database.read");
            assert_eq!(effects_section.effects[2].name, "console");
        } else {
            panic!("Expected Snippets program");
        }
    }
}
