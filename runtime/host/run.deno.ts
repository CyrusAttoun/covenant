/**
 * Minimal Covenant WASM runner for Deno
 *
 * Usage: deno run --allow-read run.deno.ts <file.wasm>
 *
 * Provides the runtime imports required by Covenant-compiled WASM:
 * - covenant_io.print(ptr, len) - print string from memory
 * - covenant_mem.alloc(size) - allocate memory (simple bump allocator)
 */

const wasmPath = Deno.args[0];
if (!wasmPath) {
  console.error('Usage: deno run --allow-read run.deno.ts <file.wasm>');
  Deno.exit(1);
}

// Read the WASM file
const wasmBytes = await Deno.readFile(wasmPath);

// Memory will be set after instantiation (exported from WASM module)
let memory: WebAssembly.Memory | null = null;

// Simple bump allocator state
let heapPtr = 0x10000; // Start allocations after typical data segment

const imports: WebAssembly.Imports = {
  covenant_io: {
    /**
     * Print a string from WASM memory
     */
    print: (ptr: number, len: number) => {
      if (!memory) {
        console.error('[runtime] Memory not initialized');
        return;
      }
      const bytes = new Uint8Array(memory.buffer, ptr, len);
      const str = new TextDecoder().decode(bytes);
      console.log(str);
    }
  },
  covenant_mem: {
    /**
     * Allocate memory (simple bump allocator)
     */
    alloc: (size: number): number => {
      const ptr = heapPtr;
      // Align to 8 bytes
      heapPtr += (size + 7) & ~7;
      return ptr;
    }
  }
};

try {
  // Instantiate the WASM module
  const { instance } = await WebAssembly.instantiate(wasmBytes, imports);

  // Get the exported memory
  memory = instance.exports.memory as WebAssembly.Memory;
  if (!memory) {
    console.error('[runtime] WASM module does not export memory');
    Deno.exit(1);
  }

  // Find and call the main function
  const main = instance.exports.main as (() => void) | undefined;
  if (!main) {
    const exports = Object.keys(instance.exports).filter(k =>
      typeof instance.exports[k] === 'function'
    );

    if (exports.length === 0) {
      console.error('[runtime] No exported functions found');
      Deno.exit(1);
    }

    console.error(`[runtime] No 'main' function found. Available: ${exports.join(', ')}`);
    Deno.exit(1);
  }

  // Call main
  main();

} catch (err) {
  console.error('[runtime] Error:', (err as Error).message);
  if ((err as Error).stack) {
    console.error((err as Error).stack);
  }
  Deno.exit(1);
}
