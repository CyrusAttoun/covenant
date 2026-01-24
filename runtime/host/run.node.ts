/**
 * Node-compatible Covenant WASM runner (for testing when Deno is unavailable)
 *
 * Usage: npx tsx run.node.ts <file.wasm>
 */

import { readFileSync } from 'fs';

const wasmPath = process.argv[2];
if (!wasmPath) {
  console.error('Usage: npx tsx run.node.ts <file.wasm>');
  process.exit(1);
}

const wasmBytes = readFileSync(wasmPath);

let memory: WebAssembly.Memory | null = null;
let heapPtr = 0x10000;

function readStr(ptr: number, len: number): string {
  if (!memory || len === 0) return '';
  const bytes = new Uint8Array(memory.buffer, ptr, len);
  return new TextDecoder().decode(bytes);
}

function writeStr(s: string): bigint {
  const encoded = new TextEncoder().encode(s);
  const ptr = heapPtr;
  heapPtr += (encoded.length + 7) & ~7;
  if (memory) {
    new Uint8Array(memory.buffer, ptr, encoded.length).set(encoded);
  }
  return (BigInt(ptr) << 32n) | BigInt(encoded.length);
}

function writeStrArray(parts: string[]): bigint {
  const fatPtrs: bigint[] = parts.map(s => writeStr(s));
  const headerSize = 4 + fatPtrs.length * 8;
  const headerPtr = heapPtr;
  heapPtr += (headerSize + 7) & ~7;
  if (memory) {
    const view = new DataView(memory.buffer);
    view.setInt32(headerPtr, fatPtrs.length, true);
    for (let i = 0; i < fatPtrs.length; i++) {
      view.setBigInt64(headerPtr + 4 + i * 8, fatPtrs[i], true);
    }
  }
  return (BigInt(headerPtr) << 32n) | BigInt(headerSize);
}

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
  covenant_io: {
    print: (ptr: number, len: number) => {
      if (!memory) { console.error('[runtime] Memory not initialized'); return; }
      const bytes = new Uint8Array(memory.buffer, ptr, len);
      console.log(new TextDecoder().decode(bytes));
    }
  },
  covenant_mem: {
    alloc: (size: number): number => {
      const ptr = heapPtr;
      heapPtr += (size + 7) & ~7;
      return ptr;
    }
  },
  covenant_text: {
    upper: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).toUpperCase()),
    lower: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).toLowerCase()),
    trim: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).trim()),
    trim_start: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).trimStart()),
    trim_end: (ptr: number, len: number): bigint => writeStr(readStr(ptr, len).trimEnd()),
    str_reverse: (ptr: number, len: number): bigint => writeStr([...readStr(ptr, len)].reverse().join('')),
    str_len: (ptr: number, len: number): bigint => BigInt([...readStr(ptr, len)].length),
    byte_len: (_ptr: number, len: number): bigint => BigInt(len),
    is_empty: (_ptr: number, len: number): bigint => len === 0 ? 1n : 0n,
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
    slice: (ptr: number, len: number, start: number, end: number): bigint =>
      writeStr([...readStr(ptr, len)].slice(start, end).join('')),
    char_at: (ptr: number, len: number, idx: number): bigint => {
      const ch = [...readStr(ptr, len)][idx] ?? '';
      return writeStr(ch);
    },
    replace: (sp: number, sl: number, fp: number, fl: number, tp: number, tl: number): bigint =>
      writeStr(readStr(sp, sl).replace(readStr(fp, fl), readStr(tp, tl))),
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
    regex_test: (pp: number, pl: number, ip: number, il: number): bigint => {
      try { return new RegExp(readStr(pp, pl)).test(readStr(ip, il)) ? 1n : 0n; }
      catch { return 0n; }
    },
    regex_match: (pp: number, pl: number, ip: number, il: number): bigint => {
      try {
        const match = readStr(ip, il).match(new RegExp(readStr(pp, pl)));
        if (!match) return 0n;
        return writeStr(JSON.stringify({ matched: match[0], index: match.index, groups: match.slice(1) }));
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
  }
};

(async () => {
  try {
    const { instance } = await WebAssembly.instantiate(wasmBytes, imports);
    memory = instance.exports.memory as WebAssembly.Memory;
    if (!memory) { console.error('[runtime] No memory export'); process.exit(1); }
    const main = instance.exports.main as (() => void) | undefined;
    if (!main) { console.error('[runtime] No main export'); process.exit(1); }
    main();
  } catch (err) {
    console.error('[runtime] Error:', (err as Error).message);
    if ((err as Error).stack) console.error((err as Error).stack);
    process.exit(1);
  }
})();
