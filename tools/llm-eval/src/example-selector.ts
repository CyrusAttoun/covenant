/**
 * Example Selector for Covenant LLM Code Generation.
 *
 * Selects 2-3 representative examples based on the generation task type.
 * Target: 1,000-1,500 tokens total for selected examples.
 */

import type { Example, TaskType } from "./types.ts";

/** Example catalog with categorization. */
const EXAMPLES: readonly Example[] = [
  {
    path: "examples/01-hello-world.cov",
    title: "Hello World (Minimal Effectful Function)",
    categories: ["effectful_function"],
    approxTokens: 80,
    priority: 3,
  },
  {
    path: "examples/02-pure-functions.cov",
    title: "Pure Functions (No Effects)",
    categories: ["pure_function"],
    approxTokens: 350,
    priority: 9,
  },
  {
    path: "examples/04-error-handling.cov",
    title: "Error Handling (Union Returns, Handle Blocks)",
    categories: ["error_handling", "type_definition"],
    approxTokens: 800,
    priority: 9,
  },
  {
    path: "examples/10-pattern-matching.cov",
    title: "Pattern Matching (Match, Destructuring)",
    categories: ["pattern_matching", "type_definition"],
    approxTokens: 550,
    priority: 8,
  },
  {
    path: "examples/16-database-dialects.cov",
    title: "Database Dialects (SQL Body Blocks)",
    categories: ["query_sql", "database_binding", "crud_operation", "transaction"],
    approxTokens: 1200,
    priority: 8,
  },
  {
    path: "examples/14-project-queries.cov",
    title: "Project Queries (Covenant Dialect)",
    categories: ["query_covenant"],
    approxTokens: 650,
    priority: 7,
  },
  {
    path: "examples/06-database-access.cov",
    title: "Database Access (Simple CRUD)",
    categories: ["crud_operation", "effectful_function"],
    approxTokens: 450,
    priority: 6,
  },
  {
    path: "examples/07-multiple-effects.cov",
    title: "Multiple Effects (Database + Network)",
    categories: ["effectful_function"],
    approxTokens: 420,
    priority: 6,
  },
  {
    path: "examples/11-extern-bindings.cov",
    title: "External Bindings (Tool Integration)",
    categories: ["effectful_function"],
    approxTokens: 700,
    priority: 5,
  },
  {
    path: "examples/13-database-module.cov",
    title: "Database Module (Schema Definition)",
    categories: ["database_binding"],
    approxTokens: 700,
    priority: 7,
  },
  {
    path: "examples/15-ast-mutations.cov",
    title: "AST Mutations (Refactoring)",
    categories: ["refactoring"],
    approxTokens: 800,
    priority: 5,
  },
];

/** Related category lookup for scoring. */
const RELATED_CATEGORIES: Partial<Record<TaskType, readonly TaskType[]>> = {
  pure_function: ["pattern_matching"],
  effectful_function: ["error_handling"],
  crud_operation: ["query_sql", "query_covenant", "database_binding"],
  error_handling: ["pattern_matching", "type_definition"],
  pattern_matching: ["type_definition", "error_handling"],
  query_sql: ["crud_operation", "database_binding"],
  query_covenant: ["crud_operation"],
  transaction: ["crud_operation", "query_sql"],
};

/** Score an example's relevance to a task type (0-100). */
function scoreExample(example: Example, taskType: TaskType): number {
  let score = 0;

  if (example.categories.includes(taskType)) {
    score += 50;
  }

  const related = RELATED_CATEGORIES[taskType] ?? [];
  for (const cat of example.categories) {
    if (related.includes(cat)) {
      score += 10;
    }
  }

  score += example.priority * 2;

  if (example.approxTokens < 500) {
    score += 5;
  }

  return score;
}

/** Infer task type from a natural language description. */
export function inferTaskType(description: string): TaskType {
  const desc = description.toLowerCase();

  if (["pure", "no effect", "calculation", "compute"].some((kw) => desc.includes(kw))) {
    return "pure_function";
  }
  if (["create", "insert", "update", "delete", "crud"].some((kw) => desc.includes(kw))) {
    return "crud_operation";
  }
  if (["error", "handle", "exception", "union"].some((kw) => desc.includes(kw))) {
    return "error_handling";
  }
  if (["match", "pattern", "destructur"].some((kw) => desc.includes(kw))) {
    return "pattern_matching";
  }
  if (["query", "select", "sql"].some((kw) => desc.includes(kw))) {
    if (["postgres", "mysql", "sql"].some((kw) => desc.includes(kw))) {
      return "query_sql";
    }
    return "query_covenant";
  }
  if (["database", "schema", "table"].some((kw) => desc.includes(kw))) {
    return "database_binding";
  }
  if (["transaction", "atomic"].some((kw) => desc.includes(kw))) {
    return "transaction";
  }
  if (["struct", "enum", "type"].some((kw) => desc.includes(kw))) {
    return "type_definition";
  }
  if (["migrate", "translate", "convert"].some((kw) => desc.includes(kw))) {
    return "migration";
  }

  return "general";
}

/**
 * Selects optimal examples for a given task.
 */
export class ExampleSelector {
  private readonly examples: readonly Example[];
  private readonly examplesDir: string;

  constructor(examplesDir?: string) {
    this.examples = EXAMPLES;
    this.examplesDir = examplesDir ?? new URL("../../examples", import.meta.url).pathname;
  }

  /**
   * Select best examples for a task type within a token budget.
   */
  select(taskType: TaskType, maxTokens = 1500, maxExamples = 3): Example[] {
    const scored: Array<{ score: number; example: Example }> = [];

    for (const example of this.examples) {
      const score = scoreExample(example, taskType);
      if (score > 0) {
        scored.push({ score, example });
      }
    }

    scored.sort((a, b) => b.score - a.score);

    const selected: Example[] = [];
    let totalTokens = 0;

    for (const { example } of scored) {
      if (selected.length >= maxExamples) break;
      if (totalTokens + example.approxTokens <= maxTokens) {
        selected.push(example);
        totalTokens += example.approxTokens;
      }
    }

    return selected;
  }

  /**
   * Load and format selected examples as markdown for LLM context.
   */
  async loadExamples(selected: readonly Example[]): Promise<string> {
    const output: string[] = ["# Example Covenant Code\n"];

    for (let i = 0; i < selected.length; i++) {
      const ex = selected[i]!;
      const filePath = `${this.examplesDir}/${ex.path}`;
      try {
        const content = await Deno.readTextFile(filePath);
        output.push(`## Example ${i + 1}: ${ex.title}\n`);
        output.push(`\`\`\`covenant\n${content}\n\`\`\`\n`);
      } catch {
        output.push(`## Example ${i + 1}: ${ex.title}\n`);
        output.push(`(File not found: ${filePath})\n`);
      }
    }

    return output.join("\n");
  }
}

/**
 * Convenience function: select and load examples for a task description.
 */
export async function selectExamplesForTask(
  description: string,
  taskType?: TaskType,
  maxTokens = 1500,
): Promise<string> {
  const selector = new ExampleSelector();
  const resolvedType = taskType ?? inferTaskType(description);
  const selected = selector.select(resolvedType, maxTokens);
  return selector.loadExamples(selected);
}
