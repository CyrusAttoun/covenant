//! Descriptions for each example file that LLMs should generate

pub struct ExampleTest {
    pub file: &'static str,
    pub description: &'static str,
}

pub const EXAMPLES: &[ExampleTest] = &[
    ExampleTest {
        file: "01-hello-world.cov",
        description: r#"A simple hello world function that:
- Has a 'console' effect
- Has a main function with no parameters that returns Unit
- Calls println with the message "Hello, world!"
"#,
    },
    ExampleTest {
        file: "02-pure-functions.cov",
        description: r#"Three pure math functions (no effects section):
1. add: Takes two Int parameters (a, b), returns their sum
2. factorial: Takes Int n, returns factorial using recursion with if/else for n <= 1 base case
3. double: Takes Int x, returns x multiplied by 2
"#,
    },
    ExampleTest {
        file: "03-file-io.cov",
        description: r#"A file transformation function that:
- Has 'filesystem' and 'console' effects
- main function returns union of Unit or IoError
- Reads from "input.txt", converts to uppercase, writes to "output.txt"
- Prints "Done!" when complete
"#,
    },
    ExampleTest {
        file: "04-error-handling.cov",
        description: r#"Error handling with:
1. ParseError enum with variants: InvalidFormat(message: String), MissingField(name: String), OutOfRange(field: String, value: Int)
2. Config struct with fields: host: String, port: Int, debug: Bool
3. parse_config pure function that takes input: String and returns union of Config or ParseError
"#,
    },
    ExampleTest {
        file: "05-http-server.cov",
        description: r#"HTTP server with:
- 'network' effect
- Request struct with: method: String, path: String, body: String
- Response struct with: status: Int, body: String
- handle_request function that takes Request and returns Response
- Matches on path to return different responses
"#,
    },
    ExampleTest {
        file: "06-database-access.cov",
        description: r#"Database access with:
- User struct with id: Int, name: String, email: String
- 'database' effect
- get_user function: takes id: Int, returns union of User (optional) or DbError
- Uses a query step with target="users", select all, where equals field="id" var="id"
"#,
    },
    ExampleTest {
        file: "07-multiple-effects.cov",
        description: r#"Function combining multiple effects:
- 'filesystem', 'network', 'console' effects
- sync_file function that reads a local file and uploads to a remote server
- Returns union of Unit or SyncError
"#,
    },
    ExampleTest {
        file: "08-effect-granularity.cov",
        description: r#"Fine-grained effects demonstration:
- 'filesystem.read' and 'filesystem.write' as separate effects
- read_only function with only read effect
- write_only function with only write effect
- Shows effect granularity for permission control
"#,
    },
    ExampleTest {
        file: "09-higher-order.cov",
        description: r#"Higher-order functions:
- map function that takes a list and a function, applies the function to each element
- filter function that takes a list and a predicate, returns matching elements
- Uses step kind="call" with function parameters
"#,
    },
    ExampleTest {
        file: "10-pattern-matching.cov",
        description: r#"Pattern matching with match steps:
- Result enum with Ok(value: T) and Err(error: E) variants
- process function that matches on a Result
- Uses step kind="match" with case handlers
"#,
    },
    ExampleTest {
        file: "11-extern-bindings.cov",
        description: r#"External function bindings:
- snippet kind="extern" for declaring external functions
- http.get binding with network effect, cost_hint=moderate, latency_hint=slow
- Declares contract="axios.get@1"
"#,
    },
    ExampleTest {
        file: "12-using-bindings.cov",
        description: r#"Using external bindings:
- Function that uses the http.get extern binding
- Makes an HTTP request and parses the response
- Demonstrates calling external tools
"#,
    },
    ExampleTest {
        file: "13-database-module.cov",
        description: r#"Database module with schema:
- snippet kind="database" with dialect="postgres"
- Schema section defining users table with id, email, name fields
- connection="env:DATABASE_URL"
"#,
    },
    ExampleTest {
        file: "14-project-queries.cov",
        description: r#"Querying the project AST:
- Function that queries target="project"
- Selects functions that have the 'database' effect
- Uses Covenant query dialect (not SQL)
"#,
    },
    ExampleTest {
        file: "15-ast-mutations.cov",
        description: r#"AST mutation operations:
- 'metaprogramming' effect for modifying source code
- Function that inserts a new node into the project
- Uses step kind="insert" with target="project.data_nodes"
"#,
    },
    ExampleTest {
        file: "16-database-dialects.cov",
        description: r#"SQL dialect queries:
- Query step with dialect="postgres" (or mysql, sqlserver, sqlite)
- Uses body ... end block with raw SQL
- params section for parameter binding
- returns annotation for result type
"#,
    },
    ExampleTest {
        file: "17-advanced-sql.cov",
        description: r#"Complex SQL query:
- Postgres dialect with JOIN, GROUP BY, subqueries
- Multiple params with different types
- Complex return type with aggregations
"#,
    },
    ExampleTest {
        file: "19-data-nodes.cov",
        description: r#"Data/documentation nodes:
- snippet kind="data" for storing structured information
- metadata section with type, tags, visibility
- content section with the actual data
- Queryable via project queries
"#,
    },
    ExampleTest {
        file: "20-knowledge-base.cov",
        description: r#"Knowledge base with multiple data nodes:
- Several snippet kind="data" entries
- Cross-references between nodes using related_to
- Different content types (prose, structured data)
"#,
    },
];
