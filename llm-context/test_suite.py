"""
Comprehensive Test Suite for Covenant LLM Generation

Defines 100+ test tasks covering various complexity levels and patterns.
"""

from generation_harness import GenerationTask
from example_selector import TaskType


def create_test_suite() -> list:
    """Create comprehensive test suite with 100+ tasks"""
    tasks = []

    # ========================================
    # Pure Functions (15 tasks)
    # ========================================

    # Simple arithmetic
    for i, (name, op, desc) in enumerate([
        ("add", "addition", "Add two integers"),
        ("subtract", "subtraction", "Subtract two integers"),
        ("multiply", "multiplication", "Multiply two integers"),
        ("divide", "division", "Divide two integers"),
        ("modulo", "modulo", "Calculate modulo of two integers"),
    ], 1):
        tasks.append(GenerationTask(
            id=f"pure_{i:03d}_{name}",
            task_type=TaskType.PURE_FUNCTION,
            description=desc,
            module="math",
            function_name=name,
            parameters=[
                {"name": "a", "type": "Int"},
                {"name": "b", "type": "Int"}
            ],
            return_type="Int",
            requirements=[
                {"priority": "high", "text": f"Must perform {op} of a and b"}
            ]
        ))

    # String operations
    for i, (name, desc) in enumerate([
        ("concat", "Concatenate two strings"),
        ("length", "Get length of a string"),
        ("uppercase", "Convert string to uppercase"),
        ("contains", "Check if string contains substring"),
    ], 6):
        tasks.append(GenerationTask(
            id=f"pure_{i:03d}_{name}",
            task_type=TaskType.PURE_FUNCTION,
            description=desc,
            module="string",
            function_name=name,
            parameters=[{"name": "s", "type": "String"}]
            if "length" in name or "uppercase" in name
            else [{"name": "s1", "type": "String"}, {"name": "s2", "type": "String"}],
            return_type="Int" if "length" in name else "Bool" if "contains" in name else "String",
            requirements=[
                {"priority": "high", "text": desc}
            ]
        ))

    # Recursive functions
    for i, (name, desc, param_type, return_type) in enumerate([
        ("factorial", "Calculate factorial", "Int", "Int"),
        ("fibonacci", "Calculate nth Fibonacci number", "Int", "Int"),
        ("sum_list", "Sum all elements in list", "List<Int>", "Int"),
        ("reverse_list", "Reverse a list", "List<Int>", "List<Int>"),
        ("max_in_list", "Find maximum in list", "List<Int>", "Int"),
    ], 10):
        tasks.append(GenerationTask(
            id=f"pure_{i:03d}_{name}",
            task_type=TaskType.PURE_FUNCTION,
            description=desc,
            module="math" if "factorial" in name or "fib" in name else "list",
            function_name=name,
            parameters=[{"name": "n" if "Int" in param_type else "items", "type": param_type}],
            return_type=return_type,
            requirements=[
                {"priority": "high", "text": desc}
            ]
        ))

    # ========================================
    # CRUD Operations (20 tasks)
    # ========================================

    entities = ["user", "product", "order", "customer"]
    operations = [
        ("create", "Create new {entity}", "union of {Entity} and DbError"),
        ("get_by_id", "Get {entity} by ID", "union of {Entity} (optional) and DbError"),
        ("update", "Update {entity} by ID", "union of {Entity} and DbError"),
        ("delete", "Delete {entity} by ID", "union of Bool and DbError"),
        ("list_all", "List all {entity}s", "union of collection of {Entity} and DbError"),
    ]

    task_id = 1
    for entity in entities:
        for op_name, desc_template, return_template in operations:
            Entity = entity.capitalize()
            desc = desc_template.format(entity=entity, Entity=Entity)
            return_type = return_template.format(entity=entity, Entity=Entity)

            params = [{"name": "id", "type": "Int"}] if "by_id" in op_name or op_name == "update" else []
            if op_name == "create":
                params = [
                    {"name": "name", "type": "String"},
                    {"name": "email", "type": "String"} if entity == "user" else {"name": "price", "type": "Float"}
                ]
            elif op_name == "update":
                params.append({"name": "data", "type": Entity})

            tasks.append(GenerationTask(
                id=f"crud_{task_id:03d}_{entity}_{op_name}",
                task_type=TaskType.CRUD_OPERATION,
                description=desc,
                module=entity,
                function_name=op_name,
                parameters=params,
                return_type=return_type,
                requirements=[
                    {"priority": "critical", "text": desc}
                ],
                expected_effects=["database"]
            ))
            task_id += 1

    # ========================================
    # Error Handling (15 tasks)
    # ========================================

    for i, (name, desc) in enumerate([
        ("parse_int", "Parse string to integer with error handling"),
        ("parse_float", "Parse string to float with error handling"),
        ("validate_email", "Validate email format"),
        ("validate_url", "Validate URL format"),
        ("divide_safe", "Divide with zero check"),
        ("parse_json", "Parse JSON string with error handling"),
        ("parse_config", "Parse config file with validation"),
        ("validate_password", "Validate password strength"),
        ("parse_date", "Parse date string with format check"),
        ("validate_phone", "Validate phone number format"),
        ("safe_access", "Safely access array element by index"),
        ("validate_range", "Validate number is in range"),
        ("parse_bool", "Parse string to boolean"),
        ("validate_hex", "Validate hexadecimal string"),
        ("safe_substring", "Safely extract substring"),
    ], 1):
        tasks.append(GenerationTask(
            id=f"error_{i:03d}_{name}",
            task_type=TaskType.ERROR_HANDLING,
            description=desc,
            module="validation" if "validate" in name else "parser",
            function_name=name,
            parameters=[{"name": "input", "type": "String"}] + (
                [{"name": "min", "type": "Int"}, {"name": "max", "type": "Int"}]
                if "range" in name else
                [{"name": "index", "type": "Int"}]
                if "access" in name else
                []
            ),
            return_type="union of Int and ParseError" if "int" in name
            else "union of Float and ParseError" if "float" in name
            else "union of Bool and ValidationError",
            requirements=[
                {"priority": "high", "text": desc},
                {"priority": "medium", "text": "Must provide helpful error messages"}
            ]
        ))

    # ========================================
    # Pattern Matching (10 tasks)
    # ========================================

    for i, (name, desc, param_type) in enumerate([
        ("json_type_name", "Get JSON value type name", "Json"),
        ("option_to_string", "Convert Option to string", "Option<String>"),
        ("result_unwrap_or", "Unwrap Result or return default", "Result<Int, Error>"),
        ("list_head", "Get first element of list", "List<Int>"),
        ("either_value", "Extract value from Either", "Either<Int, String>"),
        ("json_get_string", "Extract string from JSON", "Json"),
        ("tree_depth", "Calculate tree depth", "Tree<Int>"),
        ("option_map", "Map function over Option", "Option<Int>"),
        ("result_map_error", "Map error in Result", "Result<Int, String>"),
        ("variant_name", "Get enum variant name", "Status"),
    ], 1):
        tasks.append(GenerationTask(
            id=f"match_{i:03d}_{name}",
            task_type=TaskType.PATTERN_MATCHING,
            description=desc,
            module="util",
            function_name=name,
            parameters=[{"name": "value", "type": param_type}],
            return_type="String" if "name" in name else "Int" if "depth" in name else "Option<String>",
            requirements=[
                {"priority": "high", "text": desc},
                {"priority": "critical", "text": "Must handle all cases exhaustively"}
            ]
        ))

    # ========================================
    # Effectful Functions (15 tasks)
    # ========================================

    effect_tasks = [
        ("read_file", "Read file contents", "filesystem", "String", "FilePath"),
        ("write_file", "Write content to file", "filesystem", "Unit", "FilePath, String"),
        ("http_get", "Make HTTP GET request", "network", "Response", "String"),
        ("http_post", "Make HTTP POST request", "network", "Response", "String, String"),
        ("log_message", "Log message to console", "stdio", "Unit", "String"),
        ("random_int", "Generate random integer", "random", "Int", ""),
        ("current_timestamp", "Get current timestamp", "time", "Int", ""),
        ("sleep", "Sleep for duration", "time", "Unit", "Int"),
        ("send_email", "Send email", "network", "Unit", "String, String, String"),
        ("download_file", "Download file from URL", "network, filesystem", "Unit", "String, String"),
        ("execute_shell", "Execute shell command", "system", "String", "String"),
        ("read_env", "Read environment variable", "system", "Option<String>", "String"),
        ("create_directory", "Create directory", "filesystem", "Unit", "String"),
        ("list_directory", "List directory contents", "filesystem", "List<String>", "String"),
        ("delete_file", "Delete file", "filesystem", "Unit", "String"),
    ]

    for i, (name, desc, effects, return_type, params_str) in enumerate(effect_tasks, 1):
        params = []
        if params_str:
            for param in params_str.split(", "):
                parts = param.split()
                params.append({"name": parts[0].lower(), "type": parts[0] if len(parts) == 1 else "String"})

        tasks.append(GenerationTask(
            id=f"effect_{i:03d}_{name}",
            task_type=TaskType.EFFECTFUL_FUNCTION,
            description=desc,
            module="io" if "read" in name or "write" in name else "http" if "http" in name else "sys",
            function_name=name,
            parameters=params,
            return_type=f"union of {return_type} and IoError",
            requirements=[
                {"priority": "high", "text": desc}
            ],
            expected_effects=effects.split(", ")
        ))

    # ========================================
    # Complex Multi-Step Functions (15 tasks)
    # ========================================

    complex_tasks = [
        ("register_user", "Register new user with validation and email", "user", "database, network",
         "String, String, String", "User"),
        ("process_payment", "Process payment and update order", "payment", "database, network",
         "Int, Float", "Receipt"),
        ("import_csv", "Import CSV file to database", "import", "filesystem, database",
         "String", "Int"),
        ("generate_report", "Generate and save report", "report", "database, filesystem",
         "String", "String"),
        ("sync_data", "Sync data between databases", "sync", "database, network",
         "", "Int"),
        ("batch_update", "Batch update multiple records", "batch", "database",
         "List<Int>, String", "Int"),
        ("search_and_filter", "Search with multiple filters", "search", "database",
         "String, List<String>", "List<Result>"),
        ("aggregate_stats", "Calculate aggregate statistics", "analytics", "database",
         "String, String", "Stats"),
        ("validate_and_save", "Validate data and save to DB", "data", "database",
         "Data", "Data"),
        ("migrate_users", "Migrate users to new schema", "migration", "database",
         "", "Int"),
        ("cache_query", "Execute query with caching", "cache", "database, memory",
         "String", "List<Row>"),
        ("audit_log", "Create audit log entry", "audit", "database",
         "String, String, String", "Unit"),
        ("rate_limit_check", "Check and update rate limit", "ratelimit", "database, time",
         "String", "Bool"),
        ("webhook_handler", "Handle incoming webhook", "webhook", "network, database",
         "String, String", "Unit"),
        ("scheduled_cleanup", "Clean up old records", "cleanup", "database, time",
         "", "Int"),
    ]

    for i, (name, desc, module, effects, params_str, return_type) in enumerate(complex_tasks, 1):
        params = []
        if params_str:
            for j, param_type in enumerate(params_str.split(", ")):
                params.append({
                    "name": f"param{j+1}" if param_type in ["String", "Int", "Float"] else param_type.lower(),
                    "type": param_type
                })

        tasks.append(GenerationTask(
            id=f"complex_{i:03d}_{name}",
            task_type=TaskType.EFFECTFUL_FUNCTION,
            description=desc,
            module=module,
            function_name=name,
            parameters=params,
            return_type=f"union of {return_type} and Error",
            requirements=[
                {"priority": "high", "text": desc},
                {"priority": "medium", "text": "Must handle errors gracefully"}
            ],
            expected_effects=effects.split(", ")
        ))

    # ========================================
    # Query Tasks (15 tasks)
    # ========================================

    query_tasks = [
        ("find_active_users", "Find all active users", "covenant", "User"),
        ("count_orders_by_user", "Count orders for user", "postgres", "Int"),
        ("top_products", "Get top selling products", "postgres", "Product"),
        ("recent_activity", "Get recent user activity", "postgres", "Activity"),
        ("search_by_tag", "Search items by tag", "covenant", "Item"),
        ("aggregate_sales", "Aggregate sales by region", "postgres", "SalesData"),
        ("join_user_orders", "Join users with their orders", "postgres", "UserOrder"),
        ("window_ranking", "Rank items with window function", "sqlserver", "Ranking"),
        ("full_text_search", "Full text search in content", "postgres", "SearchResult"),
        ("geo_nearest", "Find nearest locations", "postgres", "Location"),
        ("time_series", "Query time series data", "postgres", "TimeSeries"),
        ("graph_traverse", "Traverse relationship graph", "covenant", "Node"),
        ("complex_join", "Complex multi-table join", "postgres", "JoinResult"),
        ("subquery_filter", "Filter with subquery", "postgres", "FilteredItem"),
        ("json_query", "Query JSON column", "postgres", "JsonResult"),
    ]

    for i, (name, desc, dialect, return_type) in enumerate(query_tasks, 1):
        task_type = TaskType.QUERY_SQL if dialect != "covenant" else TaskType.QUERY_COVENANT

        tasks.append(GenerationTask(
            id=f"query_{i:03d}_{name}",
            task_type=task_type,
            description=desc,
            module="query",
            function_name=name,
            parameters=[{"name": "filter", "type": "String"}],
            return_type=f"collection of {return_type}",
            requirements=[
                {"priority": "high", "text": desc}
            ],
            expected_effects=["database"],
            context=f"Use {dialect} dialect"
        ))

    return tasks


def get_test_suite_by_category(category: TaskType) -> list:
    """Get all tasks for a specific category"""
    all_tasks = create_test_suite()
    return [t for t in all_tasks if t.task_type == category]


def get_test_suite_sample(n: int = 20) -> list:
    """Get a random sample of n tasks"""
    import random
    all_tasks = create_test_suite()
    return random.sample(all_tasks, min(n, len(all_tasks)))


if __name__ == "__main__":
    suite = create_test_suite()
    print(f"Total test tasks: {len(suite)}")
    print()

    # Count by category
    from collections import Counter
    categories = Counter(t.task_type for t in suite)
    print("Tasks by category:")
    for cat, count in categories.most_common():
        print(f"  {cat.value}: {count}")
    print()

    # Show first few tasks
    print("Sample tasks:")
    for task in suite[:5]:
        print(f"  {task.id}: {task.description}")
