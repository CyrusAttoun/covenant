/**
 * Symbol Metadata Embedding Test
 *
 * Tests that symbol metadata is correctly embedded in WASM modules via the
 * _cov_get_symbol_metadata export function.
 *
 * Run with: deno run --allow-read examples/51-test.ts
 */

// Load the compiled WASM
const wasmBytes = await Deno.readFile("./examples/51-symbol-metadata-test.wasm");
const module = new WebAssembly.Module(wasmBytes);

// Get the required imports from the module
const imports = WebAssembly.Module.imports(module);
console.log("Required imports:", imports.map(i => `${i.module}.${i.name}`).join(", "));

// Simple heap allocator for testing
let heapPtr = 65536; // Start allocation after 64KB
const allocate = (size: number): number => {
  const ptr = heapPtr;
  heapPtr += size;
  return ptr;
};

// Create a stub function for unimplemented imports
const stubFn = (..._args: unknown[]) => 0;

// Build imports object dynamically
const importObject: Record<string, Record<string, unknown>> = {};
for (const imp of imports) {
  if (!importObject[imp.module]) {
    importObject[imp.module] = {};
  }
  if (imp.kind === "function") {
    // Special handling for known functions
    if (imp.module === "mem" && imp.name === "alloc") {
      importObject[imp.module][imp.name] = allocate;
    } else {
      importObject[imp.module][imp.name] = stubFn;
    }
  }
}

const instance = new WebAssembly.Instance(module, importObject);

// Get symbol metadata via exported function
const getMetadata = instance.exports._cov_get_symbol_metadata as () => bigint;
const fatPtr = getMetadata();
const offset = Number(fatPtr >> 32n);
const len = Number(fatPtr & 0xFFFFFFFFn);

console.log(`Symbol metadata at offset ${offset}, length ${len} bytes`);

// Read JSON from WASM memory
const memory = instance.exports.memory as WebAssembly.Memory;
const bytes = new Uint8Array(memory.buffer, offset, len);
const json = new TextDecoder().decode(bytes);

// Parse and display
interface EmbeddableSymbol {
  id: string;
  kind: string;
  line: number;
  calls: string[];
  references: string[];
  called_by: string[];
  referenced_by: string[];
  effects: string[];
  effect_closure: string[];
  requirements: string[];
  tests: string[];
  covers: string[];
}

const symbols: EmbeddableSymbol[] = JSON.parse(json);
console.log(`\nFound ${symbols.length} symbols:\n`);

// Display each symbol
for (const sym of symbols) {
  console.log(`--- ${sym.id} (${sym.kind}) ---`);
  if (sym.effects.length > 0) {
    console.log(`  Effects: ${sym.effects.join(", ")}`);
  }
  if (sym.effect_closure.length > 0) {
    console.log(`  Effect closure: ${sym.effect_closure.join(", ")}`);
  }
  if (sym.calls.length > 0) {
    console.log(`  Calls: ${sym.calls.join(", ")}`);
  }
  if (sym.called_by.length > 0) {
    console.log(`  Called by: ${sym.called_by.join(", ")}`);
  }
  if (sym.requirements.length > 0) {
    console.log(`  Requirements: ${sym.requirements.join(", ")}`);
  }
  if (sym.tests.length > 0) {
    console.log(`  Tests: ${sym.tests.join(", ")}`);
  }
  if (sym.covers.length > 0) {
    console.log(`  Covers: ${sym.covers.join(", ")}`);
  }
  console.log();
}

// Run assertions
let passed = 0;
let failed = 0;

function assert(condition: boolean, message: string) {
  if (condition) {
    console.log(`\x1b[32m✓\x1b[0m ${message}`);
    passed++;
  } else {
    console.log(`\x1b[31m✗\x1b[0m ${message}`);
    failed++;
  }
}

console.log("\n=== Running Assertions ===\n");

// Find specific symbols
const logMessage = symbols.find((s) => s.id === "test.log_message");
const makeGreeting = symbols.find((s) => s.id === "test.make_greeting");
const greetAndLog = symbols.find((s) => s.id === "test.greet_and_log");
const consolePrintln = symbols.find((s) => s.id === "console.println");

// Basic symbol existence
assert(logMessage !== undefined, "test.log_message symbol exists");
assert(makeGreeting !== undefined, "test.make_greeting symbol exists");
assert(greetAndLog !== undefined, "test.greet_and_log symbol exists");
assert(consolePrintln !== undefined, "console.println symbol exists");

// Requirements
assert(
  logMessage?.requirements.includes("R-001") ?? false,
  "test.log_message has R-001 requirement"
);
assert(
  logMessage?.requirements.includes("R-002") ?? false,
  "test.log_message has R-002 requirement"
);
assert(
  (logMessage?.requirements.length ?? 0) === 2,
  "test.log_message has exactly 2 requirements"
);

// Tests
assert(
  logMessage?.tests.includes("T-001") ?? false,
  "test.log_message has T-001 test"
);
assert(
  logMessage?.tests.includes("T-002") ?? false,
  "test.log_message has T-002 test"
);
assert(
  (logMessage?.tests.length ?? 0) === 2,
  "test.log_message has exactly 2 tests"
);

// Covers (test -> requirement relationship)
assert(
  logMessage?.covers.includes("R-001") ?? false,
  "test.log_message covers R-001"
);
assert(
  logMessage?.covers.includes("R-002") ?? false,
  "test.log_message covers R-002"
);

// Effects
assert(
  logMessage?.effects.includes("console") ?? false,
  "test.log_message has console effect"
);
assert(
  consolePrintln?.effects.includes("console") ?? false,
  "console.println has console effect"
);
assert(
  (makeGreeting?.effects.length ?? 1) === 0,
  "test.make_greeting is pure (no effects)"
);

// Call graph
assert(
  logMessage?.calls.includes("console.println") ?? false,
  "test.log_message calls console.println"
);
assert(
  greetAndLog?.calls.includes("test.make_greeting") ?? false,
  "test.greet_and_log calls test.make_greeting"
);
assert(
  greetAndLog?.calls.includes("test.log_message") ?? false,
  "test.greet_and_log calls test.log_message"
);

// Effect closure (transitive effects)
assert(
  greetAndLog?.effect_closure.includes("console") ?? false,
  "test.greet_and_log has console in effect closure"
);

// Called by (backward references)
assert(
  consolePrintln?.called_by.includes("test.log_message") ?? false,
  "console.println is called by test.log_message"
);

// Summary
console.log(`\n=== Results: ${passed} passed, ${failed} failed ===\n`);

if (failed > 0) {
  Deno.exit(1);
}
