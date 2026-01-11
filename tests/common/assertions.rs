use covenant_ast::*;

/// Assert that parsing succeeds
pub fn assert_parses(source: &str) {
    covenant_parser::parse(source)
        .expect("Expected source to parse successfully");
}

/// Assert that parsing fails
pub fn assert_parse_fails(source: &str) {
    assert!(
        covenant_parser::parse(source).is_err(),
        "Expected source to fail parsing"
    );
}

/// Assert AST contains a snippet with given ID
pub fn assert_has_snippet(program: &Program, id: &str) -> &Snippet {
    program.snippets
        .iter()
        .find(|s| s.id == id)
        .expect(&format!("Expected snippet with id: {}", id))
}
