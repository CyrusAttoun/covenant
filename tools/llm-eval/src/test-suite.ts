/**
 * Comprehensive Test Suite for Covenant LLM Generation.
 *
 * Defines 100+ test tasks covering various complexity levels and patterns.
 */

import type { GenerationTask, TaskType } from "./types.ts";

/** Create the full test suite with 100+ tasks. */
export function createTestSuite(): GenerationTask[] {
  const tasks: GenerationTask[] = [];

  // ========================================
  // Pure Functions (15 tasks)
  // ========================================

  const arithmetic: Array<[string, string, string]> = [
    ["add", "addition", "Add two integers"],
    ["subtract", "subtraction", "Subtract two integers"],
    ["multiply", "multiplication", "Multiply two integers"],
    ["divide", "division", "Divide two integers"],
    ["modulo", "modulo", "Calculate modulo of two integers"],
  ];

  for (let i = 0; i < arithmetic.length; i++) {
    const [name, op, desc] = arithmetic[i]!;
    tasks.push({
      id: `pure_${String(i + 1).padStart(3, "0")}_${name}`,
      taskType: "pure_function",
      description: desc,
      module: "math",
      functionName: name,
      parameters: [
        { name: "a", type: "Int" },
        { name: "b", type: "Int" },
      ],
      returnType: "Int",
      requirements: [{ priority: "high", text: `Must perform ${op} of a and b` }],
    });
  }

  const stringOps: Array<[string, string]> = [
    ["concat", "Concatenate two strings"],
    ["length", "Get length of a string"],
    ["uppercase", "Convert string to uppercase"],
    ["contains", "Check if string contains substring"],
  ];

  for (let i = 0; i < stringOps.length; i++) {
    const [name, desc] = stringOps[i]!;
    const isSingle = name === "length" || name === "uppercase";
    tasks.push({
      id: `pure_${String(i + 6).padStart(3, "0")}_${name}`,
      taskType: "pure_function",
      description: desc,
      module: "string",
      functionName: name,
      parameters: isSingle
        ? [{ name: "s", type: "String" }]
        : [{ name: "s1", type: "String" }, { name: "s2", type: "String" }],
      returnType: name === "length" ? "Int" : name === "contains" ? "Bool" : "String",
      requirements: [{ priority: "high", text: desc }],
    });
  }

  const recursive: Array<[string, string, string, string]> = [
    ["factorial", "Calculate factorial", "Int", "Int"],
    ["fibonacci", "Calculate nth Fibonacci number", "Int", "Int"],
    ["sum_list", "Sum all elements in list", "List<Int>", "Int"],
    ["reverse_list", "Reverse a list", "List<Int>", "List<Int>"],
    ["max_in_list", "Find maximum in list", "List<Int>", "Int"],
  ];

  for (let i = 0; i < recursive.length; i++) {
    const [name, desc, paramType, returnType] = recursive[i]!;
    tasks.push({
      id: `pure_${String(i + 10).padStart(3, "0")}_${name}`,
      taskType: "pure_function",
      description: desc,
      module: name.includes("factorial") || name.includes("fib") ? "math" : "list",
      functionName: name,
      parameters: [{ name: paramType === "Int" ? "n" : "items", type: paramType }],
      returnType,
      requirements: [{ priority: "high", text: desc }],
    });
  }

  // ========================================
  // CRUD Operations (20 tasks)
  // ========================================

  const entities = ["user", "product", "order", "customer"];
  const operations: Array<[string, string, string]> = [
    ["create", "Create new {entity}", "union of {Entity} and DbError"],
    ["get_by_id", "Get {entity} by ID", "union of {Entity} (optional) and DbError"],
    ["update", "Update {entity} by ID", "union of {Entity} and DbError"],
    ["delete", "Delete {entity} by ID", "union of Bool and DbError"],
    ["list_all", "List all {entity}s", "union of collection of {Entity} and DbError"],
  ];

  let crudId = 1;
  for (const entity of entities) {
    const Entity = entity.charAt(0).toUpperCase() + entity.slice(1);
    for (const [opName, descTemplate, returnTemplate] of operations) {
      const desc = descTemplate.replace("{entity}", entity).replace("{Entity}", Entity);
      const returnType = returnTemplate.replace("{entity}", entity).replace("{Entity}", Entity);

      let params: Array<{ name: string; type: string }>;
      if (opName === "create") {
        params = entity === "user"
          ? [{ name: "name", type: "String" }, { name: "email", type: "String" }]
          : [{ name: "name", type: "String" }, { name: "price", type: "Float" }];
      } else if (opName === "update") {
        params = [{ name: "id", type: "Int" }, { name: "data", type: Entity }];
      } else if (opName === "get_by_id" || opName === "delete") {
        params = [{ name: "id", type: "Int" }];
      } else {
        params = [];
      }

      tasks.push({
        id: `crud_${String(crudId).padStart(3, "0")}_${entity}_${opName}`,
        taskType: "crud_operation",
        description: desc,
        module: entity,
        functionName: opName,
        parameters: params,
        returnType,
        requirements: [{ priority: "critical", text: desc }],
        expectedEffects: ["database"],
      });
      crudId++;
    }
  }

  // ========================================
  // Error Handling (15 tasks)
  // ========================================

  const errorTasks: Array<[string, string]> = [
    ["parse_int", "Parse string to integer with error handling"],
    ["parse_float", "Parse string to float with error handling"],
    ["validate_email", "Validate email format"],
    ["validate_url", "Validate URL format"],
    ["divide_safe", "Divide with zero check"],
    ["parse_json", "Parse JSON string with error handling"],
    ["parse_config", "Parse config file with validation"],
    ["validate_password", "Validate password strength"],
    ["parse_date", "Parse date string with format check"],
    ["validate_phone", "Validate phone number format"],
    ["safe_access", "Safely access array element by index"],
    ["validate_range", "Validate number is in range"],
    ["parse_bool", "Parse string to boolean"],
    ["validate_hex", "Validate hexadecimal string"],
    ["safe_substring", "Safely extract substring"],
  ];

  for (let i = 0; i < errorTasks.length; i++) {
    const [name, desc] = errorTasks[i]!;
    const extraParams = name.includes("range")
      ? [{ name: "min", type: "Int" }, { name: "max", type: "Int" }]
      : name.includes("access")
        ? [{ name: "index", type: "Int" }]
        : [];

    const returnType = name.includes("int")
      ? "union of Int and ParseError"
      : name.includes("float")
        ? "union of Float and ParseError"
        : "union of Bool and ValidationError";

    tasks.push({
      id: `error_${String(i + 1).padStart(3, "0")}_${name}`,
      taskType: "error_handling",
      description: desc,
      module: name.includes("validate") ? "validation" : "parser",
      functionName: name,
      parameters: [{ name: "input", type: "String" }, ...extraParams],
      returnType,
      requirements: [
        { priority: "high", text: desc },
        { priority: "medium", text: "Must provide helpful error messages" },
      ],
    });
  }

  // ========================================
  // Pattern Matching (10 tasks)
  // ========================================

  const matchTasks: Array<[string, string, string]> = [
    ["json_type_name", "Get JSON value type name", "Json"],
    ["option_to_string", "Convert Option to string", "Option<String>"],
    ["result_unwrap_or", "Unwrap Result or return default", "Result<Int, Error>"],
    ["list_head", "Get first element of list", "List<Int>"],
    ["either_value", "Extract value from Either", "Either<Int, String>"],
    ["json_get_string", "Extract string from JSON", "Json"],
    ["tree_depth", "Calculate tree depth", "Tree<Int>"],
    ["option_map", "Map function over Option", "Option<Int>"],
    ["result_map_error", "Map error in Result", "Result<Int, String>"],
    ["variant_name", "Get enum variant name", "Status"],
  ];

  for (let i = 0; i < matchTasks.length; i++) {
    const [name, desc, paramType] = matchTasks[i]!;
    const returnType = name.includes("name") ? "String"
      : name.includes("depth") ? "Int"
      : "Option<String>";

    tasks.push({
      id: `match_${String(i + 1).padStart(3, "0")}_${name}`,
      taskType: "pattern_matching",
      description: desc,
      module: "util",
      functionName: name,
      parameters: [{ name: "value", type: paramType }],
      returnType,
      requirements: [
        { priority: "high", text: desc },
        { priority: "critical", text: "Must handle all cases exhaustively" },
      ],
    });
  }

  // ========================================
  // Effectful Functions (15 tasks)
  // ========================================

  const effectTasks: Array<[string, string, string, string, string]> = [
    ["read_file", "Read file contents", "filesystem", "String", "FilePath"],
    ["write_file", "Write content to file", "filesystem", "Unit", "FilePath,String"],
    ["http_get", "Make HTTP GET request", "network", "Response", "String"],
    ["http_post", "Make HTTP POST request", "network", "Response", "String,String"],
    ["log_message", "Log message to console", "stdio", "Unit", "String"],
    ["random_int", "Generate random integer", "random", "Int", ""],
    ["current_timestamp", "Get current timestamp", "time", "Int", ""],
    ["sleep", "Sleep for duration", "time", "Unit", "Int"],
    ["send_email", "Send email", "network", "Unit", "String,String,String"],
    ["download_file", "Download file from URL", "network,filesystem", "Unit", "String,String"],
    ["execute_shell", "Execute shell command", "system", "String", "String"],
    ["read_env", "Read environment variable", "system", "Option<String>", "String"],
    ["create_directory", "Create directory", "filesystem", "Unit", "String"],
    ["list_directory", "List directory contents", "filesystem", "List<String>", "String"],
    ["delete_file", "Delete file", "filesystem", "Unit", "String"],
  ];

  for (let i = 0; i < effectTasks.length; i++) {
    const [name, desc, effects, returnType, paramsStr] = effectTasks[i]!;
    const params = paramsStr
      ? paramsStr.split(",").map((p, j) => ({ name: `param${j + 1}`, type: p.trim() || "String" }))
      : [];

    tasks.push({
      id: `effect_${String(i + 1).padStart(3, "0")}_${name}`,
      taskType: "effectful_function",
      description: desc,
      module: name.includes("read") || name.includes("write") || name.includes("directory") || name.includes("delete")
        ? "io"
        : name.includes("http") || name.includes("download") || name.includes("email")
          ? "http"
          : "sys",
      functionName: name,
      parameters: params,
      returnType: `union of ${returnType} and IoError`,
      requirements: [{ priority: "high", text: desc }],
      expectedEffects: effects.split(",").map((e) => e.trim()),
    });
  }

  // ========================================
  // Complex Multi-Step Functions (15 tasks)
  // ========================================

  const complexTasks: Array<[string, string, string, string, string, string]> = [
    ["register_user", "Register new user with validation and email", "user", "database,network", "String,String,String", "User"],
    ["process_payment", "Process payment and update order", "payment", "database,network", "Int,Float", "Receipt"],
    ["import_csv", "Import CSV file to database", "import", "filesystem,database", "String", "Int"],
    ["generate_report", "Generate and save report", "report", "database,filesystem", "String", "String"],
    ["sync_data", "Sync data between databases", "sync", "database,network", "", "Int"],
    ["batch_update", "Batch update multiple records", "batch", "database", "List<Int>,String", "Int"],
    ["search_and_filter", "Search with multiple filters", "search", "database", "String,List<String>", "List<Result>"],
    ["aggregate_stats", "Calculate aggregate statistics", "analytics", "database", "String,String", "Stats"],
    ["validate_and_save", "Validate data and save to DB", "data", "database", "Data", "Data"],
    ["migrate_users", "Migrate users to new schema", "migration", "database", "", "Int"],
    ["cache_query", "Execute query with caching", "cache", "database,memory", "String", "List<Row>"],
    ["audit_log", "Create audit log entry", "audit", "database", "String,String,String", "Unit"],
    ["rate_limit_check", "Check and update rate limit", "ratelimit", "database,time", "String", "Bool"],
    ["webhook_handler", "Handle incoming webhook", "webhook", "network,database", "String,String", "Unit"],
    ["scheduled_cleanup", "Clean up old records", "cleanup", "database,time", "", "Int"],
  ];

  for (let i = 0; i < complexTasks.length; i++) {
    const [name, desc, module, effects, paramsStr, returnType] = complexTasks[i]!;
    const params = paramsStr
      ? paramsStr.split(",").map((p, j) => ({ name: `param${j + 1}`, type: p.trim() }))
      : [];

    tasks.push({
      id: `complex_${String(i + 1).padStart(3, "0")}_${name}`,
      taskType: "effectful_function",
      description: desc,
      module,
      functionName: name,
      parameters: params,
      returnType: `union of ${returnType} and Error`,
      requirements: [
        { priority: "high", text: desc },
        { priority: "medium", text: "Must handle errors gracefully" },
      ],
      expectedEffects: effects.split(",").map((e) => e.trim()),
    });
  }

  // ========================================
  // Query Tasks (15 tasks)
  // ========================================

  const queryTasks: Array<[string, string, string, string]> = [
    ["find_active_users", "Find all active users", "covenant", "User"],
    ["count_orders_by_user", "Count orders for user", "postgres", "Int"],
    ["top_products", "Get top selling products", "postgres", "Product"],
    ["recent_activity", "Get recent user activity", "postgres", "Activity"],
    ["search_by_tag", "Search items by tag", "covenant", "Item"],
    ["aggregate_sales", "Aggregate sales by region", "postgres", "SalesData"],
    ["join_user_orders", "Join users with their orders", "postgres", "UserOrder"],
    ["window_ranking", "Rank items with window function", "sqlserver", "Ranking"],
    ["full_text_search", "Full text search in content", "postgres", "SearchResult"],
    ["geo_nearest", "Find nearest locations", "postgres", "Location"],
    ["time_series", "Query time series data", "postgres", "TimeSeries"],
    ["graph_traverse", "Traverse relationship graph", "covenant", "Node"],
    ["complex_join", "Complex multi-table join", "postgres", "JoinResult"],
    ["subquery_filter", "Filter with subquery", "postgres", "FilteredItem"],
    ["json_query", "Query JSON column", "postgres", "JsonResult"],
  ];

  for (let i = 0; i < queryTasks.length; i++) {
    const [name, desc, dialect, returnType] = queryTasks[i]!;
    const taskType: TaskType = dialect !== "covenant" ? "query_sql" : "query_covenant";

    tasks.push({
      id: `query_${String(i + 1).padStart(3, "0")}_${name}`,
      taskType,
      description: desc,
      module: "query",
      functionName: name,
      parameters: [{ name: "filter", type: "String" }],
      returnType: `collection of ${returnType}`,
      requirements: [{ priority: "high", text: desc }],
      expectedEffects: ["database"],
      context: `Use ${dialect} dialect`,
    });
  }

  return tasks;
}

/** Get all tasks for a specific category. */
export function getTestSuiteByCategory(category: TaskType): GenerationTask[] {
  return createTestSuite().filter((t) => t.taskType === category);
}

/** Get a random sample of n tasks. */
export function getTestSuiteSample(n: number): GenerationTask[] {
  const all = createTestSuite();
  const count = Math.min(n, all.length);

  // Fisher-Yates shuffle on a copy, then take first n
  const shuffled = [...all];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j]!, shuffled[i]!];
  }

  return shuffled.slice(0, count);
}
