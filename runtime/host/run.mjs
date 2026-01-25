#!/usr/bin/env node
/**
 * Covenant WASM runner for Node.js
 *
 * Usage: node run.mjs <file.wasm>
 *
 * Provides the runtime imports required by Covenant-compiled WASM:
 * - mem.alloc(size) - allocate memory (simple bump allocator)
 * - console.* - console output functions
 * - text.* - string operations
 * - "std.text".* - regex operations
 * - list.* - list operations
 * - map.* - map operations
 * - fs.* - filesystem operations
 * - path.* - path operations
 */

import { readFile, writeFile, mkdir, readdir, stat, rm, copyFile, rename } from 'fs/promises';
import { existsSync } from 'fs';
import { argv, stdout, stderr } from 'process';
import { join, extname, basename, dirname, isAbsolute } from 'path';

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

// ===== String helpers for WASM <-> host communication =====

/** Read a UTF-8 string from WASM memory */
function readStr(ptr, len) {
  if (!memory || len === 0) return '';
  const bytes = new Uint8Array(memory.buffer, ptr, len);
  return new TextDecoder().decode(bytes);
}

/** Write a string into WASM memory, returns i64 fat pointer as bigint: (offset << 32) | len */
function writeStr(s) {
  const encoded = new TextEncoder().encode(s);
  const ptr = heapPtr;
  heapPtr += (encoded.length + 7) & ~7; // 8-byte aligned
  if (memory) {
    new Uint8Array(memory.buffer, ptr, encoded.length).set(encoded);
  }
  return (BigInt(ptr) << 32n) | BigInt(encoded.length);
}

/** Write an array of strings into WASM memory as [count:i32][fat_ptr_1:i64]...[fat_ptr_n:i64] */
function writeStrArray(parts) {
  // First write each string, collecting fat pointers
  const fatPtrs = parts.map(s => writeStr(s));

  // Write header: 4 bytes for count + 8 bytes per fat pointer
  const headerSize = 4 + fatPtrs.length * 8;
  const headerPtr = heapPtr;
  heapPtr += (headerSize + 7) & ~7;

  if (memory) {
    const view = new DataView(memory.buffer);
    view.setInt32(headerPtr, fatPtrs.length, true); // little-endian count
    for (let i = 0; i < fatPtrs.length; i++) {
      view.setBigInt64(headerPtr + 4 + i * 8, fatPtrs[i], true);
    }
  }
  return (BigInt(headerPtr) << 32n) | BigInt(headerSize);
}

/** Read an array of strings from WASM memory (reverse of writeStrArray) */
function readStrArray(ptr, _len) {
  if (!memory) return [];
  const view = new DataView(memory.buffer);
  const count = view.getInt32(ptr, true);
  const result = [];
  for (let i = 0; i < count; i++) {
    const fatPtr = view.getBigInt64(ptr + 4 + i * 8, true);
    const strPtr = Number(fatPtr >> 32n);
    const strLen = Number(fatPtr & 0xFFFFFFFFn);
    result.push(readStr(strPtr, strLen));
  }
  return result;
}

const imports = {
  mem: {
    alloc: (size) => {
      const ptr = heapPtr;
      heapPtr += (size + 7) & ~7;
      return ptr;
    }
  },
  console: {
    println: (ptr, len) => { console.log(readStr(ptr, len)); },
    print: (ptr, len) => { stdout.write(readStr(ptr, len)); },
    error: (ptr, len) => { console.error(readStr(ptr, len)); },
    info: (ptr, len) => { console.info(readStr(ptr, len)); },
    debug: (ptr, len) => { console.debug(readStr(ptr, len)); },
    warn: (ptr, len) => { console.warn(readStr(ptr, len)); },
  },
  text: {
    // Unary -> String
    upper: (ptr, len) => writeStr(readStr(ptr, len).toUpperCase()),
    lower: (ptr, len) => writeStr(readStr(ptr, len).toLowerCase()),
    trim: (ptr, len) => writeStr(readStr(ptr, len).trim()),
    trim_start: (ptr, len) => writeStr(readStr(ptr, len).trimStart()),
    trim_end: (ptr, len) => writeStr(readStr(ptr, len).trimEnd()),
    str_reverse: (ptr, len) => writeStr([...readStr(ptr, len)].reverse().join('')),

    // Unary -> Int/Bool
    str_len: (ptr, len) => BigInt([...readStr(ptr, len)].length),
    byte_len: (_ptr, len) => BigInt(len),
    is_empty: (_ptr, len) => len === 0 ? 1n : 0n,

    // Binary String ops
    concat: (p1, l1, p2, l2) =>
      writeStr(readStr(p1, l1) + readStr(p2, l2)),
    contains: (p1, l1, p2, l2) =>
      readStr(p1, l1).includes(readStr(p2, l2)) ? 1n : 0n,
    starts_with: (p1, l1, p2, l2) =>
      readStr(p1, l1).startsWith(readStr(p2, l2)) ? 1n : 0n,
    ends_with: (p1, l1, p2, l2) =>
      readStr(p1, l1).endsWith(readStr(p2, l2)) ? 1n : 0n,
    index_of: (p1, l1, p2, l2) =>
      BigInt(readStr(p1, l1).indexOf(readStr(p2, l2))),

    // Slice/CharAt
    slice: (ptr, len, start, end) =>
      writeStr([...readStr(ptr, len)].slice(start, end).join('')),
    char_at: (ptr, len, idx) => {
      const ch = [...readStr(ptr, len)][idx] ?? '';
      return writeStr(ch);
    },

    // Multi-arg string ops
    replace: (sp, sl, fp, fl, tp, tl) =>
      writeStr(readStr(sp, sl).replace(readStr(fp, fl), readStr(tp, tl))),
    split: (ptr, len, dp, dl) =>
      writeStrArray(readStr(ptr, len).split(readStr(dp, dl))),
    join: (ap, al, sp, sl) =>
      writeStr(readStrArray(ap, al).join(readStr(sp, sl))),
    repeat: (ptr, len, count) =>
      writeStr(readStr(ptr, len).repeat(count)),
    pad_start: (ptr, len, targetLen, fp, fl) =>
      writeStr(readStr(ptr, len).padStart(targetLen, readStr(fp, fl))),
    pad_end: (ptr, len, targetLen, fp, fl) =>
      writeStr(readStr(ptr, len).padEnd(targetLen, readStr(fp, fl))),
  },
  "std.text": {
    regex_test: (pp, pl, ip, il) => {
      try { return new RegExp(readStr(pp, pl)).test(readStr(ip, il)) ? 1n : 0n; }
      catch { return 0n; }
    },
    regex_match: (pp, pl, ip, il) => {
      try {
        const m = readStr(ip, il).match(new RegExp(readStr(pp, pl)));
        if (!m) return 0n;
        return writeStr(JSON.stringify({
          matched: m[0],
          index: m.index,
          groups: m.slice(1),
        }));
      } catch { return 0n; }
    },
    regex_replace: (pp, pl, ip, il, rp, rl) => {
      try { return writeStr(readStr(ip, il).replace(new RegExp(readStr(pp, pl)), readStr(rp, rl))); }
      catch { return writeStr(readStr(ip, il)); }
    },
    regex_replace_all: (pp, pl, ip, il, rp, rl) => {
      try { return writeStr(readStr(ip, il).replace(new RegExp(readStr(pp, pl), 'g'), readStr(rp, rl))); }
      catch { return writeStr(readStr(ip, il)); }
    },
    regex_split: (pp, pl, ip, il) => {
      try { return writeStrArray(readStr(ip, il).split(new RegExp(readStr(pp, pl)))); }
      catch { return writeStrArray([readStr(ip, il)]); }
    },
  },
  list: {
    len: (ptr, len) => {
      if (!memory || len === 0) return 0n;
      const view = new DataView(memory.buffer);
      return BigInt(view.getInt32(ptr, true));
    },
    is_empty: (ptr, len) => {
      if (!memory || len === 0) return 1n;
      const view = new DataView(memory.buffer);
      return view.getInt32(ptr, true) === 0 ? 1n : 0n;
    },
    get: (ptr, len, idx) => {
      if (!memory || len === 0) return 0n;
      const view = new DataView(memory.buffer);
      const count = view.getInt32(ptr, true);
      if (idx < 0 || idx >= count) return 0n;
      return view.getBigInt64(ptr + 4 + idx * 8, true);
    },
    first: (ptr, len) => {
      if (!memory || len === 0) return 0n;
      const view = new DataView(memory.buffer);
      const count = view.getInt32(ptr, true);
      if (count === 0) return 0n;
      return view.getBigInt64(ptr + 4, true);
    },
    last: (ptr, len) => {
      if (!memory || len === 0) return 0n;
      const view = new DataView(memory.buffer);
      const count = view.getInt32(ptr, true);
      if (count === 0) return 0n;
      return view.getBigInt64(ptr + 4 + (count - 1) * 8, true);
    },
    append: (ptr, len, item_ptr, item_len) => {
      // Read existing array, append item, write new array
      const items = readStrArray(ptr, len);
      items.push(readStr(item_ptr, item_len));
      return writeStrArray(items);
    },
    contains: (ptr, len, item_ptr, item_len) => {
      const items = readStrArray(ptr, len);
      const needle = readStr(item_ptr, item_len);
      return items.includes(needle) ? 1n : 0n;
    },
    flatten: (ptr, len) => {
      // Each element in the outer array is itself a fat pointer to an inner array
      if (!memory || len === 0) return writeStrArray([]);
      const view = new DataView(memory.buffer);
      const outerCount = view.getInt32(ptr, true);
      const result = [];
      for (let i = 0; i < outerCount; i++) {
        const innerFat = view.getBigInt64(ptr + 4 + i * 8, true);
        const innerPtr = Number(innerFat >> 32n);
        const innerLen = Number(innerFat & 0xFFFFFFFFn);
        result.push(...readStrArray(innerPtr, innerLen));
      }
      return writeStrArray(result);
    },
  },
  map: {
    get: (ptr, len, key_ptr, key_len) => {
      // Maps are stored as JSON strings for now
      if (!memory || len === 0) return 0n;
      try {
        const mapStr = readStr(ptr, len);
        const obj = JSON.parse(mapStr);
        const key = readStr(key_ptr, key_len);
        const val = obj[key];
        if (val === undefined) return 0n;
        return writeStr(String(val));
      } catch { return 0n; }
    },
  },
  fs: {
    read_file: (ptr, len) => {
      try {
        const path = readStr(ptr, len);
        // Use synchronous read for WASM compatibility
        const { readFileSync } = require('fs');
        const content = readFileSync(path, 'utf-8');
        return writeStr(content);
      } catch { return 0n; }
    },
    write_file: (pp, pl, cp, cl) => {
      try {
        const path = readStr(pp, pl);
        const content = readStr(cp, cl);
        const { writeFileSync } = require('fs');
        writeFileSync(path, content);
        return 1n;
      } catch { return 0n; }
    },
    mkdir: (ptr, len, recursive) => {
      try {
        const path = readStr(ptr, len);
        const { mkdirSync } = require('fs');
        mkdirSync(path, { recursive: recursive !== 0 });
        return 1n;
      } catch { return 0n; }
    },
    exists: (ptr, len) => {
      try {
        const path = readStr(ptr, len);
        return existsSync(path) ? 1n : 0n;
      } catch { return 0n; }
    },
    remove: (ptr, len) => {
      try {
        const path = readStr(ptr, len);
        const { rmSync } = require('fs');
        rmSync(path);
        return 1n;
      } catch { return 0n; }
    },
    stat: (ptr, len) => {
      try {
        const path = readStr(ptr, len);
        const { statSync } = require('fs');
        const info = statSync(path);
        return writeStr(JSON.stringify({
          size: info.size,
          isFile: info.isFile(),
          isDirectory: info.isDirectory(),
          modified: info.mtime?.getTime() ?? 0,
        }));
      } catch { return 0n; }
    },
    copy: (sp, sl, dp, dl) => {
      try {
        const { copyFileSync } = require('fs');
        copyFileSync(readStr(sp, sl), readStr(dp, dl));
        return 1n;
      } catch { return 0n; }
    },
    rename: (sp, sl, dp, dl) => {
      try {
        const { renameSync } = require('fs');
        renameSync(readStr(sp, sl), readStr(dp, dl));
        return 1n;
      } catch { return 0n; }
    },
    readDir: (ptr, len) => {
      try {
        const path = readStr(ptr, len);
        const { readdirSync } = require('fs');
        const entries = readdirSync(path);
        return writeStrArray(entries);
      } catch { return writeStrArray([]); }
    },
  },
  path: {
    join: (bp, bl, sp, sl) => {
      const base = readStr(bp, bl);
      const seg = readStr(sp, sl);
      return writeStr(join(base, seg));
    },
    extname: (ptr, len) => {
      const p = readStr(ptr, len);
      return writeStr(extname(p));
    },
    basename: (ptr, len) => {
      const p = readStr(ptr, len);
      return writeStr(basename(p));
    },
    dirname: (ptr, len) => {
      const p = readStr(ptr, len);
      return writeStr(dirname(p));
    },
    is_absolute: (ptr, len) => {
      const p = readStr(ptr, len);
      return isAbsolute(p) ? 1n : 0n;
    },
  },
  db: {
    execute_query: (_sql_ptr, _sql_len, _param_count) => {
      console.error('[runtime] Database queries not supported in Node.js runner');
      return 0;
    },
    query: (_conn, _sql_ptr, _sql_len) => {
      console.error('[db.query stub] called');
      return 0n;
    },
    connect: (_url_ptr, _url_len) => {
      console.error('[db.connect stub] called');
      return 0n;
    },
  },
  crypto: {
    sha256: (ptr, len) => {
      const input = readStr(ptr, len);
      console.error(`[crypto.sha256 stub] ${input.substring(0, 20)}...`);
      return writeStr('0'.repeat(64)); // Return mock hash
    },
    random_bytes: (length) => {
      console.error(`[crypto.random_bytes stub] ${length} bytes`);
      return writeStr('0'.repeat(Number(length) * 2)); // Return mock hex
    },
    verify_bcrypt: (_hash_ptr, _hash_len, _input_ptr, _input_len) => {
      console.error('[crypto.verify_bcrypt stub] called');
      return 1n; // Return true (verified)
    },
    hash_bcrypt: (ptr, len) => {
      const input = readStr(ptr, len);
      console.error(`[crypto.hash_bcrypt stub] ${input.substring(0, 10)}...`);
      return writeStr('$2b$10$mockhash'); // Return mock bcrypt hash
    },
  },
  http: {
    get: (urlFatPtr) => {
      // Fat pointer: high 32 bits = ptr, low 32 bits = len
      const ptr = Number(urlFatPtr >> 32n);
      const len = Number(urlFatPtr & 0xFFFFFFFFn);
      const url = readStr(ptr, len);
      console.error(`[http.get stub] ${url}`);
      // Return an empty response fat pointer (body = "")
      return writeStr('{"status":200,"body":""}');
    },
    post: (urlFatPtr, bodyFatPtr) => {
      const urlPtr = Number(urlFatPtr >> 32n);
      const urlLen = Number(urlFatPtr & 0xFFFFFFFFn);
      const url = readStr(urlPtr, urlLen);
      console.error(`[http.post stub] ${url}`);
      return writeStr('{"status":200,"body":""}');
    },
    fetch: (_url_ptr, _url_len) => {
      console.error('[runtime] HTTP fetch not supported in synchronous Node.js runner');
      return 0;
    },
  },
};

// Create a Proxy-based fallback for dynamically added extern imports
const proxyHandler = {
  get(target, prop) {
    if (prop in target) {
      return target[prop];
    }
    // Return a proxy for unknown modules that returns 0n for any function call
    return new Proxy({}, {
      get(_moduleTarget, funcName) {
        return (..._args) => {
          // Unknown function - return 0n as a safe default
          return 0n;
        };
      }
    });
  }
};

const proxiedImports = new Proxy(imports, proxyHandler);

try {
  // Instantiate the WASM module
  const { instance } = await WebAssembly.instantiate(wasmBytes, proxiedImports);

  // Get the exported memory
  memory = instance.exports.memory;
  if (!memory) {
    console.error('[runtime] WASM module does not export memory');
    process.exit(1);
  }

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
