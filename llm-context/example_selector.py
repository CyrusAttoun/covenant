"""
Example Selector for Covenant LLM Code Generation

Selects 2-3 representative examples based on the generation task type.
Target: 1,000-1,500 tokens total for selected examples.
"""

from pathlib import Path
from typing import List, Dict, Optional
from enum import Enum


class TaskType(Enum):
    """Categories of generation tasks"""
    PURE_FUNCTION = "pure_function"
    EFFECTFUL_FUNCTION = "effectful_function"
    CRUD_OPERATION = "crud_operation"
    ERROR_HANDLING = "error_handling"
    PATTERN_MATCHING = "pattern_matching"
    TYPE_DEFINITION = "type_definition"
    DATABASE_BINDING = "database_binding"
    QUERY_COVENANT = "query_covenant"
    QUERY_SQL = "query_sql"
    TRANSACTION = "transaction"
    MIGRATION = "migration"
    REFACTORING = "refactoring"
    GENERAL = "general"


class Example:
    """Represents a Covenant example file"""

    def __init__(self, path: str, title: str, categories: List[TaskType],
                 approx_tokens: int, priority: int = 5):
        self.path = path
        self.title = title
        self.categories = categories
        self.approx_tokens = approx_tokens
        self.priority = priority  # 1-10, higher = more important

    def __repr__(self):
        return f"Example({self.title}, {self.approx_tokens} tokens, priority={self.priority})"


# Example catalog with categorization
EXAMPLES = [
    Example(
        path="examples/01-hello-world.cov",
        title="Hello World (Minimal Effectful Function)",
        categories=[TaskType.EFFECTFUL_FUNCTION],
        approx_tokens=80,
        priority=3
    ),
    Example(
        path="examples/02-pure-functions.cov",
        title="Pure Functions (No Effects)",
        categories=[TaskType.PURE_FUNCTION],
        approx_tokens=350,
        priority=9
    ),
    Example(
        path="examples/04-error-handling.cov",
        title="Error Handling (Union Returns, Handle Blocks)",
        categories=[TaskType.ERROR_HANDLING, TaskType.TYPE_DEFINITION],
        approx_tokens=800,
        priority=9
    ),
    Example(
        path="examples/10-pattern-matching.cov",
        title="Pattern Matching (Match, Destructuring)",
        categories=[TaskType.PATTERN_MATCHING, TaskType.TYPE_DEFINITION],
        approx_tokens=550,
        priority=8
    ),
    Example(
        path="examples/16-database-dialects.cov",
        title="Database Dialects (SQL Body Blocks)",
        categories=[TaskType.QUERY_SQL, TaskType.DATABASE_BINDING,
                    TaskType.CRUD_OPERATION, TaskType.TRANSACTION],
        approx_tokens=1200,
        priority=8
    ),
    Example(
        path="examples/14-project-queries.cov",
        title="Project Queries (Covenant Dialect)",
        categories=[TaskType.QUERY_COVENANT],
        approx_tokens=650,
        priority=7
    ),
    Example(
        path="examples/06-database-access.cov",
        title="Database Access (Simple CRUD)",
        categories=[TaskType.CRUD_OPERATION, TaskType.EFFECTFUL_FUNCTION],
        approx_tokens=450,
        priority=6
    ),
    Example(
        path="examples/07-multiple-effects.cov",
        title="Multiple Effects (Database + Network)",
        categories=[TaskType.EFFECTFUL_FUNCTION],
        approx_tokens=420,
        priority=6
    ),
    Example(
        path="examples/11-extern-bindings.cov",
        title="External Bindings (Tool Integration)",
        categories=[TaskType.EFFECTFUL_FUNCTION],
        approx_tokens=700,
        priority=5
    ),
    Example(
        path="examples/13-database-module.cov",
        title="Database Module (Schema Definition)",
        categories=[TaskType.DATABASE_BINDING],
        approx_tokens=700,
        priority=7
    ),
    Example(
        path="examples/15-ast-mutations.cov",
        title="AST Mutations (Refactoring)",
        categories=[TaskType.REFACTORING],
        approx_tokens=800,
        priority=5
    ),
]


class ExampleSelector:
    """Selects optimal examples for a given task"""

    def __init__(self, examples_dir: Optional[Path] = None):
        self.examples = EXAMPLES
        self.examples_dir = examples_dir or Path(__file__).parent.parent / "examples"

    def select(self, task_type: TaskType, max_tokens: int = 1500,
               max_examples: int = 3) -> List[Example]:
        """
        Select best examples for a task type.

        Args:
            task_type: Type of generation task
            max_tokens: Maximum total tokens for all examples
            max_examples: Maximum number of examples to return

        Returns:
            List of selected examples, sorted by relevance
        """
        # Score each example
        scored = []
        for ex in self.examples:
            score = self._score_example(ex, task_type)
            if score > 0:
                scored.append((score, ex))

        # Sort by score (descending)
        scored.sort(key=lambda x: x[0], reverse=True)

        # Select examples within token budget
        selected = []
        total_tokens = 0

        for score, ex in scored:
            if len(selected) >= max_examples:
                break
            if total_tokens + ex.approx_tokens <= max_tokens:
                selected.append(ex)
                total_tokens += ex.approx_tokens

        return selected

    def _score_example(self, example: Example, task_type: TaskType) -> float:
        """
        Score an example's relevance to a task type.

        Returns:
            Score (0-100), higher = more relevant
        """
        score = 0.0

        # Primary match: example category matches task type
        if task_type in example.categories:
            score += 50

        # Secondary matches: related categories
        related = self._get_related_categories(task_type)
        for cat in example.categories:
            if cat in related:
                score += 10

        # Priority boost
        score += example.priority * 2

        # Token efficiency: prefer concise examples
        if example.approx_tokens < 500:
            score += 5

        return score

    def _get_related_categories(self, task_type: TaskType) -> List[TaskType]:
        """Get categories related to a task type"""
        relations = {
            TaskType.PURE_FUNCTION: [TaskType.PATTERN_MATCHING],
            TaskType.EFFECTFUL_FUNCTION: [TaskType.ERROR_HANDLING],
            TaskType.CRUD_OPERATION: [TaskType.QUERY_SQL, TaskType.QUERY_COVENANT,
                                      TaskType.DATABASE_BINDING],
            TaskType.ERROR_HANDLING: [TaskType.PATTERN_MATCHING, TaskType.TYPE_DEFINITION],
            TaskType.PATTERN_MATCHING: [TaskType.TYPE_DEFINITION, TaskType.ERROR_HANDLING],
            TaskType.QUERY_SQL: [TaskType.CRUD_OPERATION, TaskType.DATABASE_BINDING],
            TaskType.QUERY_COVENANT: [TaskType.CRUD_OPERATION],
            TaskType.TRANSACTION: [TaskType.CRUD_OPERATION, TaskType.QUERY_SQL],
        }
        return relations.get(task_type, [])

    def load_examples(self, selected: List[Example]) -> str:
        """
        Load and format selected examples for LLM context.

        Args:
            selected: List of examples to load

        Returns:
            Formatted string with all examples
        """
        output = ["# Example Covenant Code\n"]

        for i, ex in enumerate(selected, 1):
            path = self.examples_dir / ex.path
            try:
                content = path.read_text(encoding='utf-8')
                output.append(f"## Example {i}: {ex.title}\n")
                output.append(f"```covenant\n{content}\n```\n")
            except FileNotFoundError:
                output.append(f"## Example {i}: {ex.title}\n")
                output.append(f"(File not found: {path})\n")

        return "\n".join(output)

    def get_recommended_for_task(self, task_description: str) -> TaskType:
        """
        Infer task type from description (simple heuristic).

        Args:
            task_description: User's task description

        Returns:
            Inferred TaskType
        """
        desc_lower = task_description.lower()

        # Keyword matching
        if any(kw in desc_lower for kw in ["pure", "no effect", "calculation", "compute"]):
            return TaskType.PURE_FUNCTION

        if any(kw in desc_lower for kw in ["create", "insert", "update", "delete", "crud"]):
            return TaskType.CRUD_OPERATION

        if any(kw in desc_lower for kw in ["error", "handle", "exception", "union"]):
            return TaskType.ERROR_HANDLING

        if any(kw in desc_lower for kw in ["match", "pattern", "destructur"]):
            return TaskType.PATTERN_MATCHING

        if any(kw in desc_lower for kw in ["query", "select", "sql"]):
            if "postgres" in desc_lower or "mysql" in desc_lower or "sql" in desc_lower:
                return TaskType.QUERY_SQL
            return TaskType.QUERY_COVENANT

        if any(kw in desc_lower for kw in ["database", "schema", "table"]):
            return TaskType.DATABASE_BINDING

        if any(kw in desc_lower for kw in ["transaction", "atomic"]):
            return TaskType.TRANSACTION

        if any(kw in desc_lower for kw in ["struct", "enum", "type"]):
            return TaskType.TYPE_DEFINITION

        if any(kw in desc_lower for kw in ["migrate", "translate", "convert"]):
            return TaskType.MIGRATION

        # Default
        return TaskType.GENERAL


def select_examples_for_task(task_description: str,
                             task_type: Optional[TaskType] = None,
                             max_tokens: int = 1500) -> str:
    """
    Convenience function to select and load examples.

    Args:
        task_description: Description of the generation task
        task_type: Optional explicit task type (inferred if not provided)
        max_tokens: Maximum tokens for examples

    Returns:
        Formatted examples string for LLM context
    """
    selector = ExampleSelector()

    # Infer task type if not provided
    if task_type is None:
        task_type = selector.get_recommended_for_task(task_description)

    # Select examples
    selected = selector.select(task_type, max_tokens=max_tokens)

    # Load and format
    return selector.load_examples(selected)


# CLI for testing
if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        print("Usage: python example_selector.py <task_description>")
        print("\nExample:")
        print('  python example_selector.py "Generate a CRUD function for users"')
        sys.exit(1)

    task_desc = " ".join(sys.argv[1:])
    selector = ExampleSelector()

    # Infer task type
    task_type = selector.get_recommended_for_task(task_desc)
    print(f"Task Type: {task_type.value}")
    print()

    # Select examples
    selected = selector.select(task_type, max_tokens=1500, max_examples=3)
    print(f"Selected {len(selected)} examples:")
    for ex in selected:
        print(f"  - {ex.title} ({ex.approx_tokens} tokens)")
    print()

    # Show total tokens
    total = sum(ex.approx_tokens for ex in selected)
    print(f"Total: ~{total} tokens")
    print()

    # Load examples
    examples_text = selector.load_examples(selected)
    print("=" * 80)
    print(examples_text)
