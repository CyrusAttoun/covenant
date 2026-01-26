/**
 * Test script for Example 20: Knowledge Base Traversal
 *
 * This script demonstrates querying and traversing a knowledge graph
 * built with kind="data" snippets and relations.
 */

import { CovenantQueryRunner } from "../runtime/host/query-runner.ts";

async function main() {
  console.log("=== Example 20: Knowledge Base Traversal ===\n");

  const runner = new CovenantQueryRunner();

  // Load the compiled WASM module
  console.log("Loading WASM module...");
  try {
    await runner.load("./examples/20-knowledge-base.wasm");
    console.log("✓ Module loaded successfully\n");
  } catch (error) {
    console.error("✗ Failed to load WASM module:");
    console.error(error);
    Deno.exit(1);
  }

  // List all exports
  console.log("Exported functions:");
  const exports = runner.listExports();
  exports.forEach((name) => console.log(`  - ${name}`));
  console.log();

  // Inspect embedded knowledge base using GAI functions
  console.log("Embedded Knowledge Base (via GAI functions):");
  const nodeCount = runner.nodeCount();
  console.log(`  Total KB nodes: ${nodeCount}`);

  if (nodeCount > 0) {
    console.log("\n  Knowledge base structure:");
    const nodes = runner.getAllNodes();

    // Group by prefix to show hierarchy
    const byPrefix: Record<string, typeof nodes> = {};
    for (const node of nodes) {
      const prefix = node.id.split(".")[0];
      if (!byPrefix[prefix]) byPrefix[prefix] = [];
      byPrefix[prefix].push(node);
    }

    for (const [prefix, prefixNodes] of Object.entries(byPrefix)) {
      console.log(`\n  ${prefix}/ (${prefixNodes.length} nodes):`);
      prefixNodes.forEach((node) => {
        console.log(`    - ${node.id}`);
      });
    }

    console.log("\n  Sample content:");
    nodes.slice(0, 3).forEach((node) => {
      console.log(`\n    ${node.id}:`);
      const lines = node.content.split("\n").filter((l) => l.trim());
      const preview = lines.slice(0, 3).join("\n      ");
      console.log(`      ${preview}`);
    });
  }

  console.log("\n\nQuery Functions:");

  // These demonstrate the INTENDED usage - actual results pending full implementation

  console.log("\n1. find_by_topic('design')");
  console.log("   Query: Find all KB entries with 'design' in content/notes");
  console.log("   Expected: kb.design, kb.design.philosophy, kb.design.four_layers, etc.");
  console.log("   (Currently returns empty - pending implementation)\n");

  console.log("2. get_ancestors('kb.design.philosophy')");
  console.log("   Query: Traverse 'contained_by' relations transitively");
  console.log("   Expected: kb.design -> kb.root");
  console.log("   (Requires traverse step implementation)\n");

  console.log("3. get_descendants('kb.root')");
  console.log("   Query: Traverse 'contains' relations transitively");
  console.log("   Expected: All child nodes in the hierarchy");
  console.log("   (Requires traverse step implementation)\n");

  console.log("4. find_related('kb.design.philosophy', 2)");
  console.log("   Query: Find nodes within 2 hops via any relation");
  console.log("   Expected: Related design docs, FAQ entries, etc.");
  console.log("   (Requires traverse step implementation)\n");

  console.log("5. find_docs_for_code('auth.login')");
  console.log("   Query: Find data nodes with rel_to target=code_id type=describes");
  console.log("   Expected: Documentation snippets describing auth.login");
  console.log("   (Requires relation query support)\n");

  console.log("=== Test Complete ===");
  console.log("\nNotes:");
  console.log("- GAI functions successfully expose embedded knowledge graph");
  console.log("- Node IDs and content are accessible via WASM exports");
  console.log("- Query functions demonstrate intended usage patterns");
  console.log("- Full query execution pending:");
  console.log("  - compile_project_query implementation");
  console.log("  - traverse step support");
  console.log("  - relation queries (rel_to, rel_from)");
}

if (import.meta.main) {
  await main();
}
