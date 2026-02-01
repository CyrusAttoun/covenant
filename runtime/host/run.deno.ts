/**
 * Covenant WASM runner for Deno
 *
 * Usage: deno run --allow-read --allow-write run.deno.ts <file.wasm>
 *
 * Provides the runtime imports required by Covenant-compiled WASM:
 * - mem.alloc(size) - allocate memory (simple bump allocator)
 * - console.* - console output functions
 * - text.* - string operations
 * - "std.text".* - regex operations
 * - list.* - list operations
 * - fs.* - filesystem operations
 * - path.* - path operations
 */

const wasmPath = Deno.args[0];
if (!wasmPath) {
  console.error('Usage: deno run --allow-read --allow-write run.deno.ts <file.wasm>');
  Deno.exit(1);
}

// Read the WASM file
const wasmBytes = await Deno.readFile(wasmPath);

// Memory will be set after instantiation (exported from WASM module)
let memory: WebAssembly.Memory | null = null;

// Simple bump allocator state
let heapPtr = 0x10000; // Start allocations after typical data segment

// ===== String helpers for WASM ↔ host communication =====

/** Read a UTF-8 string from WASM memory */
function readStr(ptr: number, len: number): string {
  if (!memory || len === 0) return '';
  const bytes = new Uint8Array(memory.buffer, ptr, len);
  return new TextDecoder().decode(bytes);
}

/** Write a string into WASM memory, returns i64 fat pointer as bigint: (offset << 32) | len */
function writeStr(s: string): bigint {
  const encoded = new TextEncoder().encode(s);
  const ptr = heapPtr;
  heapPtr += (encoded.length + 7) & ~7; // 8-byte aligned
  if (memory) {
    new Uint8Array(memory.buffer, ptr, encoded.length).set(encoded);
  }
  return (BigInt(ptr) << 32n) | BigInt(encoded.length);
}

/** Write an array of strings into WASM memory as [count:i32][fat_ptr_1:i64]...[fat_ptr_n:i64] */
function writeStrArray(parts: string[]): bigint {
  // First write each string, collecting fat pointers
  const fatPtrs: bigint[] = parts.map(s => writeStr(s));

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
function readStrArray(ptr: number, _len: number): string[] {
  if (!memory) return [];
  const view = new DataView(memory.buffer);
  const count = view.getInt32(ptr, true);
  const result: string[] = [];
  for (let i = 0; i < count; i++) {
    const fatPtr = view.getBigInt64(ptr + 4 + i * 8, true);
    const strPtr = Number(fatPtr >> 32n);
    const strLen = Number(fatPtr & 0xFFFFFFFFn);
    result.push(readStr(strPtr, strLen));
  }
  return result;
}

const imports: WebAssembly.Imports = {
  mem: {
    alloc: (size: number): number => {
      const ptr = heapPtr;
      heapPtr += (size + 7) & ~7;
      return ptr;
    }
  },
  console: {
    println: (ptr: number, len: number) => { console.log(readStr(ptr, len)); },
    print: (ptr: number, len: number) => { Deno.stdout.writeSync(new TextEncoder().encode(readStr(ptr, len))); },
    error: (ptr: number, len: number) => { console.error(readStr(ptr, len)); },
    info: (ptr: number, len: number) => { console.info(readStr(ptr, len)); },
    debug: (ptr: number, len: number) => { console.debug(readStr(ptr, len)); },
    warn: (ptr: number, len: number) => { console.warn(readStr(ptr, len)); },
  },
  text: {
    // Unary -> String
    upper: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).toUpperCase()),
    lower: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).toLowerCase()),
    trim: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).trim()),
    trim_start: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).trimStart()),
    trim_end: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).trimEnd()),
    str_reverse: (ptr: number, len: number): bigint => writeStr([...readStr(ptr, len)].reverse().join('')),

    // Unary -> Int/Bool
    str_len: (ptr: number, len: number): bigint => BigInt([...readStr(ptr, len)].length),
    byte_len: (_ptr: number, len: number): bigint => BigInt(len),
    is_empty: (_ptr: number, len: number): bigint => len === 0 ? 1n : 0n,

    // Binary String ops
    concat: (p1: number, l1: number, p2: number, l2: number): bigint =>
      writeStr(readStr(p1, l1) + readStr(p2, l2)),
    contains: (p1: number, l1: number, p2: number, l2: number): bigint =>
      readStr(p1, l1).includes(readStr(p2, l2)) ? 1n : 0n,
    starts_with: (p1: number, l1: number, p2: number, l2: number): bigint =>
      readStr(p1, l1).startsWith(readStr(p2, l2)) ? 1n : 0n,
    ends_with: (p1: number, l1: number, p2: number, l2: number): bigint =>
      readStr(p1, l1).endsWith(readStr(p2, l2)) ? 1n : 0n,
    index_of: (p1: number, l1: number, p2: number, l2: number): bigint =>
      BigInt(readStr(p1, l1).indexOf(readStr(p2, l2))),

    // Slice/CharAt
    slice: (ptr: number, len: number, start: number, end: number): bigint =>
      writeStr([...readStr(ptr, len)].slice(start, end).join('')),
    char_at: (ptr: number, len: number, idx: number): bigint => {
      const ch = [...readStr(ptr, len)][idx] ?? '';
      return writeStr(ch);
    },

    // Multi-arg string ops
    replace: (sp: number, sl: number, fp: number, fl: number, tp: number, tl: number): bigint =>
      writeStr(readStr(sp, sl).replace(readStr(fp, fl), readStr(tp, tl))),
    replace_all: (sp: number, sl: number, fp: number, fl: number, tp: number, tl: number): bigint =>
      writeStr(readStr(sp, sl).replaceAll(readStr(fp, fl), readStr(tp, tl))),
    split: (ptr: number, len: number, dp: number, dl: number): bigint =>
      writeStrArray(readStr(ptr, len).split(readStr(dp, dl))),
    join: (ap: number, al: number, sp: number, sl: number): bigint =>
      writeStr(readStrArray(ap, al).join(readStr(sp, sl))),
    repeat: (ptr: number, len: number, count: number): bigint =>
      writeStr(readStr(ptr, len).repeat(count)),
    pad_start: (ptr: number, len: number, targetLen: number, fp: number, fl: number): bigint =>
      writeStr(readStr(ptr, len).padStart(targetLen, readStr(fp, fl))),
    pad_end: (ptr: number, len: number, targetLen: number, fp: number, fl: number): bigint =>
      writeStr(readStr(ptr, len).padEnd(targetLen, readStr(fp, fl))),
  },
  "std.text": {
    regex_test: (pp: number, pl: number, ip: number, il: number): bigint => {
      try { return new RegExp(readStr(pp, pl)).test(readStr(ip, il)) ? 1n : 0n; }
      catch { return 0n; }
    },
    regex_match: (pp: number, pl: number, ip: number, il: number): bigint => {
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
    regex_replace: (pp: number, pl: number, ip: number, il: number, rp: number, rl: number): bigint => {
      try { return writeStr(readStr(ip, il).replace(new RegExp(readStr(pp, pl)), readStr(rp, rl))); }
      catch { return writeStr(readStr(ip, il)); }
    },
    regex_replace_all: (pp: number, pl: number, ip: number, il: number, rp: number, rl: number): bigint => {
      try { return writeStr(readStr(ip, il).replace(new RegExp(readStr(pp, pl), 'g'), readStr(rp, rl))); }
      catch { return writeStr(readStr(ip, il)); }
    },
    regex_split: (pp: number, pl: number, ip: number, il: number): bigint => {
      try { return writeStrArray(readStr(ip, il).split(new RegExp(readStr(pp, pl)))); }
      catch { return writeStrArray([readStr(ip, il)]); }
    },
  },
  list: {
    len: (ptr: number, len: number): bigint => {
      if (!memory || len === 0) return 0n;
      const view = new DataView(memory.buffer);
      return BigInt(view.getInt32(ptr, true));
    },
    is_empty: (ptr: number, len: number): bigint => {
      if (!memory || len === 0) return 1n;
      const view = new DataView(memory.buffer);
      return view.getInt32(ptr, true) === 0 ? 1n : 0n;
    },
    get: (ptr: number, len: number, idx: number): bigint => {
      if (!memory || len === 0) return 0n;
      const view = new DataView(memory.buffer);
      const count = view.getInt32(ptr, true);
      if (idx < 0 || idx >= count) return 0n;
      return view.getBigInt64(ptr + 4 + idx * 8, true);
    },
    first: (ptr: number, len: number): bigint => {
      if (!memory || len === 0) return 0n;
      const view = new DataView(memory.buffer);
      const count = view.getInt32(ptr, true);
      if (count === 0) return 0n;
      return view.getBigInt64(ptr + 4, true);
    },
    last: (ptr: number, len: number): bigint => {
      if (!memory || len === 0) return 0n;
      const view = new DataView(memory.buffer);
      const count = view.getInt32(ptr, true);
      if (count === 0) return 0n;
      return view.getBigInt64(ptr + 4 + (count - 1) * 8, true);
    },
    append: (ptr: number, len: number, item_ptr: number, item_len: number): bigint => {
      // Read existing array, append item, write new array
      const items = readStrArray(ptr, len);
      items.push(readStr(item_ptr, item_len));
      return writeStrArray(items);
    },
    contains: (ptr: number, len: number, item_ptr: number, item_len: number): bigint => {
      const items = readStrArray(ptr, len);
      const needle = readStr(item_ptr, item_len);
      return items.includes(needle) ? 1n : 0n;
    },
    flatten: (ptr: number, len: number): bigint => {
      // Each element in the outer array is itself a fat pointer to an inner array
      if (!memory || len === 0) return writeStrArray([]);
      const view = new DataView(memory.buffer);
      const outerCount = view.getInt32(ptr, true);
      const result: string[] = [];
      for (let i = 0; i < outerCount; i++) {
        const innerFat = view.getBigInt64(ptr + 4 + i * 8, true);
        const innerPtr = Number(innerFat >> 32n);
        const innerLen = Number(innerFat & 0xFFFFFFFFn);
        result.push(...readStrArray(innerPtr, innerLen));
      }
      return writeStrArray(result);
    },
    find: (list_ptr: number, list_len: number, pred_ptr: number, pred_len: number): bigint => {
      // Find item matching predicate (exact string match for now)
      const items = readStrArray(list_ptr, list_len);
      const predicate = readStr(pred_ptr, pred_len);
      const found = items.find(item => item === predicate);
      return found ? writeStr(found) : 0n;
    },
  },
  map: {
    get: (ptr: number, len: number, key_ptr: number, key_len: number): bigint => {
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
    read_file: (ptr: number, len: number): bigint => {
      try {
        const path = readStr(ptr, len);
        const content = Deno.readTextFileSync(path);
        return writeStr(content);
      } catch { return 0n; }
    },
    write_file: (pp: number, pl: number, cp: number, cl: number): bigint => {
      try {
        const path = readStr(pp, pl);
        const content = readStr(cp, cl);
        Deno.writeTextFileSync(path, content);
        return 1n;
      } catch { return 0n; }
    },
    mkdir: (ptr: number, len: number, recursive: number): bigint => {
      try {
        const path = readStr(ptr, len);
        Deno.mkdirSync(path, { recursive: recursive !== 0 });
        return 1n;
      } catch { return 0n; }
    },
    exists: (ptr: number, len: number): bigint => {
      try {
        const path = readStr(ptr, len);
        Deno.statSync(path);
        return 1n;
      } catch { return 0n; }
    },
    remove: (ptr: number, len: number): bigint => {
      try {
        const path = readStr(ptr, len);
        Deno.removeSync(path);
        return 1n;
      } catch { return 0n; }
    },
    stat: (ptr: number, len: number): bigint => {
      try {
        const path = readStr(ptr, len);
        const info = Deno.statSync(path);
        return writeStr(JSON.stringify({
          size: info.size,
          isFile: info.isFile,
          isDirectory: info.isDirectory,
          modified: info.mtime?.getTime() ?? 0,
        }));
      } catch { return 0n; }
    },
    copy: (sp: number, sl: number, dp: number, dl: number): bigint => {
      try {
        Deno.copyFileSync(readStr(sp, sl), readStr(dp, dl));
        return 1n;
      } catch { return 0n; }
    },
    rename: (sp: number, sl: number, dp: number, dl: number): bigint => {
      try {
        Deno.renameSync(readStr(sp, sl), readStr(dp, dl));
        return 1n;
      } catch { return 0n; }
    },
    read_dir: (ptr: number, len: number): bigint => {
      try {
        const path = readStr(ptr, len);
        const entries: string[] = [];
        for (const entry of Deno.readDirSync(path)) {
          // Only include files for now (until struct support is added)
          if (entry.isFile) {
            entries.push(entry.name);
          }
        }
        return writeStrArray(entries);
      } catch { return writeStrArray([]); }
    },
  },
  path: {
    join: (bp: number, bl: number, sp: number, sl: number): bigint => {
      const base = readStr(bp, bl);
      const seg = readStr(sp, sl);
      // Simple path join
      const sep = base.includes('\\') ? '\\' : '/';
      const joined = base.endsWith(sep) ? base + seg : base + sep + seg;
      return writeStr(joined);
    },
    extname: (ptr: number, len: number): bigint => {
      const p = readStr(ptr, len);
      const dot = p.lastIndexOf('.');
      return writeStr(dot >= 0 ? p.slice(dot) : '');
    },
    basename: (ptr: number, len: number): bigint => {
      const p = readStr(ptr, len);
      const sep = p.includes('\\') ? '\\' : '/';
      const parts = p.split(sep);
      return writeStr(parts[parts.length - 1] || '');
    },
    dirname: (ptr: number, len: number): bigint => {
      const p = readStr(ptr, len);
      const sep = p.includes('\\') ? '\\' : '/';
      const parts = p.split(sep);
      parts.pop();
      return writeStr(parts.join(sep) || '.');
    },
    is_absolute: (ptr: number, len: number): bigint => {
      const p = readStr(ptr, len);
      return (p.startsWith('/') || /^[A-Za-z]:[\\/]/.test(p)) ? 1n : 0n;
    },
  },
  db: {
    execute_query: (_sql_ptr: number, _sql_len: number, _param_count: number): number => {
      console.error('[runtime] Database queries not supported in Deno runner');
      return 0;
    },
  },
  http: {
    fetch: (_url_ptr: number, _url_len: number): number => {
      console.error('[runtime] HTTP fetch not supported in synchronous Deno runner');
      return 0;
    },
  },
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
