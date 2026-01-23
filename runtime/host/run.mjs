#!/usr/bin/env node
/**
 * Minimal Covenant WASM runner for Node.js
 *
 * Usage: node run.mjs <file.wasm>
 *
 * Provides the runtime imports required by Covenant-compiled WASM:
 * - covenant_io.print(ptr, len) - print string from memory
 * - covenant_mem.alloc(size) - allocate memory (simple bump allocator)
 */

import { readFile } from 'fs/promises';
import { argv, stdout, stderr } from 'process';

const wasmPath = argv[2];
if (!wasmPath) {
  console.error('Usage: node run.mjs <file.wasm>');
  process.exit(1);
}

// Read the WASM file
const wasmBytes = await readFile(wasmPath);

// Memory will be set after instantiation (exported from WASM module)
let memory = null;

// Simple bump allocator state
let heapPtr = 0x10000; // Start allocations after typical data segment

const imports = {
  covenant_io: {
    /**
     * Print a string from WASM memory
     * @param {number} ptr - Pointer to string data in memory
     * @param {number} len - Length of string in bytes
     */
    print: (ptr, len) => {
      if (!memory) {
        console.error('[runtime] Memory not initialized');
        return;
      }
      const bytes = new Uint8Array(memory.buffer, ptr, len);
      const str = new TextDecoder().decode(bytes);
      stdout.write(str + '\n');
    }
  },
  covenant_mem: {
    /**
     * Allocate memory (simple bump allocator)
     * @param {number} size - Number of bytes to allocate
     * @returns {number} Pointer to allocated memory
     */
    alloc: (size) => {
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
  memory = instance.exports.memory;
  if (!memory) {
    console.error('[runtime] WASM module does not export memory');
    process.exit(1);
  }

  // Update heap pointer to be past the data segment
  // The global at index 0 contains the heap start pointer
  // For now, just use a safe default that's past typical data segments

  // Find and call the main function
  const main = instance.exports.main;
  if (!main) {
    // Try to find any exported function if there's no main
    const exports = Object.keys(instance.exports).filter(k =>
      typeof instance.exports[k] === 'function'
    );

    if (exports.length === 0) {
      console.error('[runtime] No exported functions found');
      process.exit(1);
    }

    console.error(`[runtime] No 'main' function found. Available: ${exports.join(', ')}`);
    process.exit(1);
  }

  // Call main
  main();

} catch (err) {
  console.error('[runtime] Error:', err.message);
  if (err.stack) {
    console.error(err.stack);
  }
  process.exit(1);
}
