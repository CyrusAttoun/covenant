/**
 * Covenant Query REPL
 *
 * Interactive command-line tool for querying compiled Covenant WASM modules.
 *
 * Usage:
 *   deno run --allow-read query-repl.ts
 *
 * Commands:
 *   :load <file.wasm>      - Load a compiled WASM module
 *   :query <term>          - Search all data nodes for term
 *   :query <term> --full   - Search with full chunk output
 *   :show <nodeId>         - Show full content of a node
 *   :show <nodeId> --raw   - Show with newlines preserved
 *   :list                  - List available query functions
 *   :nodes                 - List all nodes in the module
 *   :help                  - Show help
 *   :quit                  - Exit
 *
 * Direct function calls:
 *   <function>         - Call a no-arg function
 *   <function> "arg"   - Call with string argument
 */

import { CovenantQueryRunner } from "../../runtime/host/query-runner.ts";

class QueryRepl {
  private runner: CovenantQueryRunner | null = null;
  private loadedFile: string | null = null;
  private history: string[] = [];
  private historyIndex = 0;
  private encoder = new TextEncoder();
  private decoder = new TextDecoder();

  async run(): Promise<void> {
    console.log("Covenant Query REPL v0.2.0");
    console.log("Type :help for commands, :quit to exit\n");

    while (true) {
      Deno.stdout.writeSync(this.encoder.encode("query> "));

      const line = await this.readLine();
      if (line === null) break;
      if (!line.trim()) continue;

      // Add to history (don't add duplicates of the last entry)
      if (this.history.length === 0 || this.history[this.history.length - 1] !== line) {
        this.history.push(line);
      }
      this.historyIndex = this.history.length;

      try {
        if (line.startsWith(":")) {
          await this.handleCommand(line);
        } else {
          await this.executeQuery(line);
        }
      } catch (e) {
        console.error(`Error: ${e instanceof Error ? e.message : e}`);
      }
    }
  }

  private async readLine(): Promise<string | null> {
    let line = "";
    let cursorPos = 0;
    const buf = new Uint8Array(1);

    // Enable raw mode for character-by-character input
    Deno.stdin.setRaw(true);

    try {
      while (true) {
        const n = await Deno.stdin.read(buf);
        if (n === null) {
          Deno.stdin.setRaw(false);
          return null;
        }

        const byte = buf[0];

        // Ctrl+D on empty line = exit
        if (byte === 0x04 && line.length === 0) {
          console.log("");
          Deno.stdin.setRaw(false);
          return null;
        }

        // Ctrl+C = cancel current line
        if (byte === 0x03) {
          console.log("^C");
          Deno.stdin.setRaw(false);
          return "";
        }

        // Enter
        if (byte === 0x0d || byte === 0x0a) {
          console.log("");
          Deno.stdin.setRaw(false);
          return line;
        }

        // Backspace
        if (byte === 0x7f || byte === 0x08) {
          if (cursorPos > 0) {
            line = line.slice(0, cursorPos - 1) + line.slice(cursorPos);
            cursorPos--;
            this.redrawLine("query> ", line, cursorPos);
          }
          continue;
        }

        // Escape sequence (arrow keys, etc.)
        if (byte === 0x1b) {
          const seq = new Uint8Array(2);
          const n1 = await Deno.stdin.read(seq);
          if (n1 === null) continue;

          if (seq[0] === 0x5b) {
            // CSI sequence
            switch (seq[1]) {
              case 0x41: // Up arrow
                if (this.historyIndex > 0) {
                  this.historyIndex--;
                  line = this.history[this.historyIndex] || "";
                  cursorPos = line.length;
                  this.redrawLine("query> ", line, cursorPos);
                }
                break;
              case 0x42: // Down arrow
                if (this.historyIndex < this.history.length) {
                  this.historyIndex++;
                  line = this.history[this.historyIndex] || "";
                  cursorPos = line.length;
                  this.redrawLine("query> ", line, cursorPos);
                }
                break;
              case 0x43: // Right arrow
                if (cursorPos < line.length) {
                  cursorPos++;
                  Deno.stdout.writeSync(this.encoder.encode("\x1b[C"));
                }
                break;
              case 0x44: // Left arrow
                if (cursorPos > 0) {
                  cursorPos--;
                  Deno.stdout.writeSync(this.encoder.encode("\x1b[D"));
                }
                break;
            }
          }
          continue;
        }

        // Regular printable character
        if (byte >= 0x20 && byte < 0x7f) {
          const char = String.fromCharCode(byte);
          line = line.slice(0, cursorPos) + char + line.slice(cursorPos);
          cursorPos++;
          if (cursorPos === line.length) {
            Deno.stdout.writeSync(this.encoder.encode(char));
          } else {
            this.redrawLine("query> ", line, cursorPos);
          }
        }
      }
    } catch {
      Deno.stdin.setRaw(false);
      return null;
    }
  }

  private redrawLine(prompt: string, line: string, cursorPos: number): void {
    // Clear line and redraw
    Deno.stdout.writeSync(this.encoder.encode("\r\x1b[K" + prompt + line));
    // Move cursor to correct position
    if (cursorPos < line.length) {
      const moveBack = line.length - cursorPos;
      Deno.stdout.writeSync(this.encoder.encode(`\x1b[${moveBack}D`));
    }
  }

  private async handleCommand(cmd: string): Promise<void> {
    const parts = cmd.slice(1).split(/\s+/);
    const command = parts[0];
    const args = parts.slice(1).join(" ");

    switch (command) {
      case "load":
        await this.loadWasm(args);
        break;
      case "query":
        await this.searchContent(args);
        break;
      case "show":
        this.showNode(args);
        break;
      case "list":
        this.listFunctions();
        break;
      case "nodes":
        this.listNodes();
        break;
      case "help":
        this.showHelp();
        break;
      case "quit":
      case "q":
        console.log("Goodbye!");
        Deno.exit(0);
        break;
      default:
        console.log(`Unknown command: ${command}. Type :help for commands.`);
    }
  }

  private async loadWasm(path: string): Promise<void> {
    if (!path) {
      console.log("Usage: :load <file.wasm>");
      return;
    }

    this.runner = new CovenantQueryRunner();
    await this.runner.load(path);
    this.loadedFile = path;
    const nodeCount = this.runner.nodeCount();
    console.log(`Loaded ${path} (${nodeCount} nodes)`);
  }

  private async searchContent(input: string): Promise<void> {
    if (!this.runner) {
      console.log("No module loaded. Use :load <file.wasm>");
      return;
    }

    // Parse --full flag
    const fullMatch = input.match(/^(.+?)\s+--full\s*$/);
    const showFull = fullMatch !== null;
    const term = showFull ? fullMatch[1].trim() : input.trim();

    if (!term) {
      console.log("Usage: :query <search term> [--full]");
      return;
    }

    // Search all nodes for the term in their content
    const allNodes = this.runner.getAllNodes();
    const matches = allNodes.filter((node) =>
      node.content.toLowerCase().includes(term.toLowerCase())
    );

    if (matches.length === 0) {
      console.log(`No results for "${term}"`);
      return;
    }

    console.log(`Found ${matches.length} result(s):\n`);
    for (const node of matches) {
      console.log(`  [${this.getNodeKind(node.id)}] ${node.id}`);

      if (showFull) {
        // Show full content
        const content = node.content.replace(/\n/g, " ");
        console.log(`    "${content}"\n`);
      } else {
        // Find context around the match (512 bytes: 256 before + 256 after)
        const lowerContent = node.content.toLowerCase();
        const lowerTerm = term.toLowerCase();
        const idx = lowerContent.indexOf(lowerTerm);
        if (idx >= 0) {
          const start = Math.max(0, idx - 256);
          const end = Math.min(node.content.length, idx + term.length + 256);
          let snippet = node.content.slice(start, end).replace(/\n/g, " ");
          if (start > 0) snippet = "..." + snippet;
          if (end < node.content.length) snippet = snippet + "...";
          console.log(`    "${snippet}"\n`);
        }
      }
    }
  }

  private showNode(input: string): void {
    if (!this.runner) {
      console.log("No module loaded. Use :load <file.wasm>");
      return;
    }

    // Parse --raw flag
    const rawMatch = input.match(/^(.+?)\s+--raw\s*$/);
    const showRaw = rawMatch !== null;
    const nodeId = showRaw ? rawMatch[1].trim() : input.trim();

    if (!nodeId) {
      console.log("Usage: :show <nodeId> [--raw]");
      return;
    }

    // Find the node
    const allNodes = this.runner.getAllNodes();
    const node = allNodes.find((n) => n.id === nodeId);

    if (!node) {
      console.log(`Node not found: ${nodeId}`);
      return;
    }

    const kind = this.getNodeKind(node.id);
    console.log(`\n[${kind}] ${node.id}`);
    console.log("─".repeat(60));

    if (showRaw) {
      console.log(node.content);
    } else {
      console.log(node.content.replace(/\n/g, " "));
    }

    console.log("─".repeat(60));
    console.log(`(${node.content.length} bytes)\n`);
  }

  private getNodeKind(nodeId: string): string {
    if (!this.runner) return "unknown";

    const count = this.runner.nodeCount();
    for (let i = 0; i < count; i++) {
      const id = this.runner.readString(this.runner.getNodeId(i));
      if (id === nodeId) {
        try {
          return this.runner.readString(
            this.runner.call("cov_get_node_kind", i) as bigint
          );
        } catch {
          return "data";
        }
      }
    }
    return "unknown";
  }

  private listFunctions(): void {
    if (!this.runner) {
      console.log("No module loaded. Use :load <file.wasm>");
      return;
    }

    const exports = this.runner.listExports();
    const queryFns = exports.filter(
      (name) =>
        !name.startsWith("cov_") &&
        !name.startsWith("memory") &&
        name !== "main" &&
        name !== "__data_end" &&
        name !== "__heap_base"
    );

    if (queryFns.length === 0) {
      console.log("No query functions found.");
      return;
    }

    console.log("Query functions:");
    for (const fn of queryFns.sort()) {
      console.log(`  ${fn}`);
    }
  }

  private listNodes(): void {
    if (!this.runner) {
      console.log("No module loaded. Use :load <file.wasm>");
      return;
    }

    const nodes = this.runner.getAllNodes();
    console.log(`${nodes.length} nodes loaded:\n`);
    for (const node of nodes) {
      const kind = this.getNodeKind(node.id);
      const preview = node.content.slice(0, 50).replace(/\n/g, " ");
      console.log(`  [${kind}] ${node.id}`);
      if (preview) {
        console.log(`    ${preview}...`);
      }
    }
  }

  private async executeQuery(line: string): Promise<void> {
    if (!this.runner) {
      console.log("No module loaded. Use :load <file.wasm>");
      return;
    }

    // Parse: function_name or function_name "arg"
    const match = line.match(/^(\w+)(?:\s+"([^"]*)")?$/);
    if (!match) {
      console.log('Invalid syntax. Use: function_name or function_name "arg"');
      return;
    }

    const [, funcName, arg] = match;

    try {
      let result: bigint;
      if (arg !== undefined) {
        result = this.runner.queryWithString(funcName, arg);
      } else {
        result = this.runner.call(funcName) as bigint;
      }

      // Try to interpret as query results
      const nodes = this.runner.getQueryResultNodes(result);
      if (nodes.length > 0) {
        console.log(`Found ${nodes.length} result(s):\n`);
        for (const node of nodes) {
          console.log(`  [${node.kind}] ${node.id}`);
          const preview = node.content.slice(0, 60).replace(/\n/g, " ");
          if (preview) {
            console.log(`    ${preview}...`);
          }
        }
      } else {
        // Just show the raw result
        console.log(`Result: ${result}`);
      }
    } catch (e) {
      console.error(`Query error: ${e instanceof Error ? e.message : e}`);
    }
  }

  private showHelp(): void {
    console.log(`
Covenant Query REPL

Commands:
  :load <file.wasm>       Load a compiled Covenant WASM module
  :query <term>           Search all data nodes for term (512 byte context)
  :query <term> --full    Search with full chunk output
  :show <nodeId>          Show full content of a node
  :show <nodeId> --raw    Show with newlines preserved
  :list                   List available query functions
  :nodes                  List all nodes in the loaded module
  :help                   Show this help
  :quit                   Exit the REPL

Direct function calls:
  <function>              Call a no-arg query (e.g., get_all_docs)
  <function> "arg"        Call with string arg (e.g., search_by_keyword "effects")

Navigation:
  Up/Down arrows          Cycle through command history
  Left/Right arrows       Move cursor within line

Examples:
  :load output/rag-query.wasm
  :query effects
  :query effects --full
  :show doc.intro --raw
  get_all_docs
  search_by_keyword "tutorial"
`);
  }
}

const repl = new QueryRepl();
await repl.run();
