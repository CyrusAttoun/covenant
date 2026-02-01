/**
 * CovenantQueryRunner - Minimal WASM loader for Covenant query examples
 *
 * This loader provides a simple interface to:
 * 1. Load compiled Covenant WASM modules
 * 2. Call exported query functions
 * 3. Access embedded graph data via GAI functions
 *
 * Usage:
 * ```typescript
 * const runner = new CovenantQueryRunner();
 * await runner.load("./example.wasm");
 * const results = runner.call("find_docs");
 * ```
 */

export class CovenantQueryRunner {
  private instance: WebAssembly.Instance | null = null;
  private memory: WebAssembly.Memory | null = null;
  private heapPtr = 0x10000; // Simple bump allocator

  /**
   * Read a UTF-8 string from WASM memory
   */
  private readStr(ptr: number, len: number): string {
    if (!this.memory || len === 0) return "";
    const bytes = new Uint8Array(this.memory.buffer, ptr, len);
    return new TextDecoder().decode(bytes);
  }

  /**
   * Write a string into WASM memory, returns i64 fat pointer as bigint
   */
  private writeStr(s: string): bigint {
    const encoded = new TextEncoder().encode(s);
    const ptr = this.heapPtr;
    this.heapPtr += (encoded.length + 7) & ~7; // 8-byte aligned
    if (this.memory) {
      new Uint8Array(this.memory.buffer, ptr, encoded.length).set(encoded);
    }
    return (BigInt(ptr) << 32n) | BigInt(encoded.length);
  }

  /**
   * No-op stub for import functions we don't use
   */
  private noop(..._args: unknown[]): bigint {
    return 0n;
  }

  /**
   * Read a string from a fat pointer
   */
  private readStrFromFatPtr(fatPtr: bigint): string {
    const len = Number(fatPtr & 0xFFFFFFFFn);
    const ptr = Number((fatPtr >> 32n) & 0xFFFFFFFFn);
    return this.readStr(ptr, len);
  }

  /**
   * Allocate and write a collection of strings to WASM memory
   * Returns a fat pointer to the collection
   */
  private writeStringCollection(items: string[]): bigint {
    // Allocate space for count + item pointers
    const headerSize = 4 + items.length * 8;
    const headerPtr = this.heapPtr;
    this.heapPtr += (headerSize + 7) & ~7; // 8-byte align

    if (!this.memory) return 0n;

    const view = new DataView(this.memory.buffer);

    // Write count
    view.setInt32(headerPtr, items.length, true); // little-endian

    // Write each string and its fat pointer
    for (let i = 0; i < items.length; i++) {
      const fatPtr = this.writeStr(items[i]);
      const offset = headerPtr + 4 + i * 8;
      // Write i64 fat pointer as two i32s (little-endian)
      view.setInt32(offset, Number(fatPtr & 0xFFFFFFFFn), true);
      view.setInt32(offset + 4, Number((fatPtr >> 32n) & 0xFFFFFFFFn), true);
    }

    // Return fat pointer to collection
    return (BigInt(headerPtr) << 32n) | BigInt(headerSize);
  }

  /**
   * Load a compiled Covenant WASM module
   * @param wasmPath Path to the .wasm file
   */
  async load(wasmPath: string): Promise<void> {
    const wasmBuffer = await Deno.readFile(wasmPath);
    const module = await WebAssembly.compile(wasmBuffer);

    // Provide required imports (minimal stubs for functions we don't use)
    const imports: WebAssembly.Imports = {
      mem: {
        alloc: (size: number): number => {
          const ptr = this.heapPtr;
          this.heapPtr += (size + 7) & ~7; // 8-byte aligned
          return ptr;
        },
      },
      console: {
        println: (ptr: number, len: number) => {
          console.log(this.readStr(ptr, len));
        },
        print: (ptr: number, len: number) => {
          Deno.stdout.writeSync(
            new TextEncoder().encode(this.readStr(ptr, len))
          );
        },
        error: (ptr: number, len: number) => {
          console.error(this.readStr(ptr, len));
        },
        info: (ptr: number, len: number) => {
          console.info(this.readStr(ptr, len));
        },
        debug: (ptr: number, len: number) => {
          console.debug(this.readStr(ptr, len));
        },
        warn: (ptr: number, len: number) => {
          console.warn(this.readStr(ptr, len));
        },
      },
      text: {
        upper: (sPtr: number, sLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return this.writeStr(s.toUpperCase());
        },
        lower: (sPtr: number, sLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return this.writeStr(s.toLowerCase());
        },
        trim: (sPtr: number, sLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return this.writeStr(s.trim());
        },
        trim_start: (sPtr: number, sLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return this.writeStr(s.trimStart());
        },
        trim_end: (sPtr: number, sLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return this.writeStr(s.trimEnd());
        },
        str_reverse: (sPtr: number, sLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return this.writeStr([...s].reverse().join(""));
        },
        str_len: (sPtr: number, sLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return BigInt(s.length);
        },
        byte_len: (_sPtr: number, sLen: number): bigint => {
          return BigInt(sLen); // Already have byte length
        },
        is_empty: (_sPtr: number, sLen: number): bigint => {
          return sLen === 0 ? 1n : 0n;
        },
        concat: (aPtr: number, aLen: number, bPtr: number, bLen: number): bigint => {
          const a = this.readStr(aPtr, aLen);
          const b = this.readStr(bPtr, bLen);
          return this.writeStr(a + b);
        },
        contains: (sPtr: number, sLen: number, subPtr: number, subLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const sub = this.readStr(subPtr, subLen);
          return s.includes(sub) ? 1n : 0n;
        },
        starts_with: (sPtr: number, sLen: number, prefixPtr: number, prefixLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const prefix = this.readStr(prefixPtr, prefixLen);
          return s.startsWith(prefix) ? 1n : 0n;
        },
        ends_with: (sPtr: number, sLen: number, suffixPtr: number, suffixLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const suffix = this.readStr(suffixPtr, suffixLen);
          return s.endsWith(suffix) ? 1n : 0n;
        },
        index_of: (sPtr: number, sLen: number, subPtr: number, subLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const sub = this.readStr(subPtr, subLen);
          return BigInt(s.indexOf(sub));
        },
        slice: (sPtr: number, sLen: number, start: number, end: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return this.writeStr(s.slice(start, end));
        },
        char_at: (sPtr: number, sLen: number, idx: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          if (idx < 0 || idx >= s.length) return 0n;
          return this.writeStr(s.charAt(idx));
        },
        replace: (sPtr: number, sLen: number, fromPtr: number, fromLen: number, toPtr: number, toLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const from = this.readStr(fromPtr, fromLen);
          const to = this.readStr(toPtr, toLen);
          return this.writeStr(s.replace(from, to));
        },
        replace_all: (sPtr: number, sLen: number, fromPtr: number, fromLen: number, toPtr: number, toLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const from = this.readStr(fromPtr, fromLen);
          const to = this.readStr(toPtr, toLen);
          return this.writeStr(s.replaceAll(from, to));
        },
        split: (sPtr: number, sLen: number, delimPtr: number, delimLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const delim = this.readStr(delimPtr, delimLen);
          const parts = s.split(delim);
          return this.writeStringCollection(parts);
        },
        join: (delimPtr: number, delimLen: number, collFatPtr: bigint | number): bigint => {
          // collFatPtr points to: [count:i32][item0:i64]...
          const delim = this.readStr(delimPtr, delimLen);
          if (!this.memory) return 0n;
          const fatPtr = BigInt(collFatPtr);
          const collPtr = Number((fatPtr >> 32n) & 0xFFFFFFFFn);
          if (collPtr === 0) return this.writeStr("");
          const view = new DataView(this.memory.buffer);
          const count = view.getInt32(collPtr, true);
          const parts: string[] = [];
          for (let i = 0; i < count; i++) {
            const itemOffset = collPtr + 4 + i * 8;
            const lo = view.getInt32(itemOffset, true);
            const hi = view.getInt32(itemOffset + 4, true);
            const fp = (BigInt(hi) << 32n) | BigInt(lo >>> 0);
            parts.push(this.readStrFromFatPtr(fp));
          }
          return this.writeStr(parts.join(delim));
        },
        repeat: (sPtr: number, sLen: number, times: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          return this.writeStr(s.repeat(times));
        },
        pad_start: (sPtr: number, sLen: number, targetLen: number, padPtr: number, padLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const pad = this.readStr(padPtr, padLen);
          return this.writeStr(s.padStart(targetLen, pad));
        },
        pad_end: (sPtr: number, sLen: number, targetLen: number, padPtr: number, padLen: number): bigint => {
          const s = this.readStr(sPtr, sLen);
          const pad = this.readStr(padPtr, padLen);
          return this.writeStr(s.padEnd(targetLen, pad));
        },
      },
      "std.text": {
        regex_test: this.noop.bind(this),
        regex_match: this.noop.bind(this),
        regex_replace: this.noop.bind(this),
        regex_replace_all: this.noop.bind(this),
        regex_split: this.noop.bind(this),
      },
      list: {
        // List functions receive the pointer directly (not fat pointer)
        // The WASM extracts the pointer from fat pointer before calling
        len: (collPtr: number): bigint => {
          if (!this.memory || collPtr === 0) return 0n;
          const view = new DataView(this.memory.buffer);
          return BigInt(view.getInt32(collPtr, true));
        },
        is_empty: (collPtr: number): bigint => {
          if (!this.memory || collPtr === 0) return 1n;
          const view = new DataView(this.memory.buffer);
          return view.getInt32(collPtr, true) === 0 ? 1n : 0n;
        },
        get: (collPtr: number, idx: number): bigint => {
          if (!this.memory || collPtr === 0) return 0n;
          const view = new DataView(this.memory.buffer);
          const count = view.getInt32(collPtr, true);
          if (idx < 0 || idx >= count) return 0n;
          const itemOffset = collPtr + 4 + idx * 8;
          const lo = view.getInt32(itemOffset, true);
          const hi = view.getInt32(itemOffset + 4, true);
          return (BigInt(hi) << 32n) | BigInt(lo >>> 0);
        },
        first: (collPtr: number): bigint => {
          if (!this.memory || collPtr === 0) return 0n;
          const view = new DataView(this.memory.buffer);
          const count = view.getInt32(collPtr, true);
          if (count === 0) return 0n;
          const itemOffset = collPtr + 4;
          const lo = view.getInt32(itemOffset, true);
          const hi = view.getInt32(itemOffset + 4, true);
          return (BigInt(hi) << 32n) | BigInt(lo >>> 0);
        },
        last: (collPtr: number): bigint => {
          if (!this.memory || collPtr === 0) return 0n;
          const view = new DataView(this.memory.buffer);
          const count = view.getInt32(collPtr, true);
          if (count === 0) return 0n;
          const itemOffset = collPtr + 4 + (count - 1) * 8;
          const lo = view.getInt32(itemOffset, true);
          const hi = view.getInt32(itemOffset + 4, true);
          return (BigInt(hi) << 32n) | BigInt(lo >>> 0);
        },
        append: (collPtr: number, itemFatPtr: bigint): bigint => {
          if (!this.memory) return 0n;
          const view = new DataView(this.memory.buffer);
          const count = collPtr ? view.getInt32(collPtr, true) : 0;

          // Allocate new collection
          const newSize = 4 + (count + 1) * 8;
          const newPtr = this.heapPtr;
          this.heapPtr += (newSize + 7) & ~7;

          // Write new count
          view.setInt32(newPtr, count + 1, true);

          // Copy existing items
          for (let i = 0; i < count; i++) {
            const srcOffset = collPtr + 4 + i * 8;
            const dstOffset = newPtr + 4 + i * 8;
            const lo = view.getInt32(srcOffset, true);
            const hi = view.getInt32(srcOffset + 4, true);
            view.setInt32(dstOffset, lo, true);
            view.setInt32(dstOffset + 4, hi, true);
          }

          // Add new item
          const newItemOffset = newPtr + 4 + count * 8;
          view.setInt32(newItemOffset, Number(itemFatPtr & 0xFFFFFFFFn), true);
          view.setInt32(newItemOffset + 4, Number((itemFatPtr >> 32n) & 0xFFFFFFFFn), true);

          return (BigInt(newPtr) << 32n) | BigInt(newSize);
        },
        contains: (collPtr: number, itemPtr: number, itemLen: number): bigint => {
          if (!this.memory || collPtr === 0) return 0n;
          const searchStr = this.readStr(itemPtr, itemLen);
          const view = new DataView(this.memory.buffer);
          const count = view.getInt32(collPtr, true);
          for (let i = 0; i < count; i++) {
            const itemOffset = collPtr + 4 + i * 8;
            const lo = view.getInt32(itemOffset, true);
            const hi = view.getInt32(itemOffset + 4, true);
            const fp = (BigInt(hi) << 32n) | BigInt(lo >>> 0);
            if (this.readStrFromFatPtr(fp) === searchStr) return 1n;
          }
          return 0n;
        },
        flatten: (collPtr: number): bigint => {
          // For now, return a fat pointer to the same data
          if (!this.memory || collPtr === 0) return 0n;
          const view = new DataView(this.memory.buffer);
          const count = view.getInt32(collPtr, true);
          const size = 4 + count * 8;
          return (BigInt(collPtr) << 32n) | BigInt(size);
        },
      },
      map: {
        get: this.noop.bind(this),
      },
      fs: {
        read_file: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            const content = Deno.readTextFileSync(path);
            return this.writeStr(content);
          } catch {
            return 0n;
          }
        },
        write_file: (pathPtr: number, pathLen: number, contentPtr: number, contentLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          const content = this.readStr(contentPtr, contentLen);
          try {
            Deno.writeTextFileSync(path, content);
            return 1n; // Success
          } catch {
            return 0n;
          }
        },
        mkdir: (pathPtr: number, pathLen: number, recursive: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            Deno.mkdirSync(path, { recursive: recursive !== 0 });
            return 1n;
          } catch {
            // Ignore if already exists
            return 1n;
          }
        },
        exists: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            Deno.statSync(path);
            return 1n;
          } catch {
            return 0n;
          }
        },
        remove: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            Deno.removeSync(path);
            return 1n;
          } catch {
            return 0n;
          }
        },
        rename: (srcPtr: number, srcLen: number, dstPtr: number, dstLen: number): bigint => {
          const src = this.readStr(srcPtr, srcLen);
          const dst = this.readStr(dstPtr, dstLen);
          try {
            Deno.renameSync(src, dst);
            return 1n;
          } catch {
            return 0n;
          }
        },
        list_dir: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            const entries: string[] = [];
            for (const entry of Deno.readDirSync(path)) {
              entries.push(entry.name);
            }
            return this.writeStringCollection(entries);
          } catch {
            return 0n;
          }
        },
        read_dir: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            const entries: string[] = [];
            for (const entry of Deno.readDirSync(path)) {
              entries.push(entry.name);
            }
            return this.writeStringCollection(entries);
          } catch {
            return 0n;
          }
        },
        is_dir: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            return Deno.statSync(path).isDirectory ? 1n : 0n;
          } catch {
            return 0n;
          }
        },
        is_file: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            return Deno.statSync(path).isFile ? 1n : 0n;
          } catch {
            return 0n;
          }
        },
        file_size: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            return BigInt(Deno.statSync(path).size);
          } catch {
            return -1n;
          }
        },
        stat: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          try {
            return BigInt(Deno.statSync(path).size);
          } catch {
            return -1n;
          }
        },
        copy: (srcPtr: number, srcLen: number, dstPtr: number, dstLen: number): bigint => {
          const src = this.readStr(srcPtr, srcLen);
          const dst = this.readStr(dstPtr, dstLen);
          try {
            Deno.copyFileSync(src, dst);
            return 1n;
          } catch {
            return 0n;
          }
        },
      },
      path: {
        join: (basePtr: number, baseLen: number, segPtr: number, segLen: number): bigint => {
          const base = this.readStr(basePtr, baseLen);
          const segment = this.readStr(segPtr, segLen);
          const result = base.endsWith("/") ? base + segment : base + "/" + segment;
          return this.writeStr(result);
        },
        basename: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
          return this.writeStr(idx >= 0 ? path.slice(idx + 1) : path);
        },
        dirname: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
          return this.writeStr(idx >= 0 ? path.slice(0, idx) : ".");
        },
        extname: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          const idx = path.lastIndexOf(".");
          const slashIdx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
          // Only return extension if dot is after last slash
          if (idx > slashIdx) {
            return this.writeStr(path.slice(idx));
          }
          return this.writeStr("");
        },
        is_absolute: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          return path.startsWith("/") || /^[A-Za-z]:/.test(path) ? 1n : 0n;
        },
        normalize: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          // Basic normalization: remove . and resolve ..
          const parts = path.split("/").filter((p) => p !== "." && p !== "");
          const result: string[] = [];
          for (const part of parts) {
            if (part === "..") {
              result.pop();
            } else {
              result.push(part);
            }
          }
          return this.writeStr(result.join("/"));
        },
        resolve: (pathPtr: number, pathLen: number): bigint => {
          const path = this.readStr(pathPtr, pathLen);
          // For now, just return the path as-is
          return this.writeStr(path);
        },
      },
    };

    // Instantiate with imports
    this.instance = await WebAssembly.instantiate(module, imports);

    // Access exported memory if available
    if (this.instance.exports.memory instanceof WebAssembly.Memory) {
      this.memory = this.instance.exports.memory;
    }
  }

  /**
   * Call an exported function by name
   * @param functionName Name of the exported function (without module prefix)
   * @param args Function arguments
   * @returns Function result
   */
  call(functionName: string, ...args: unknown[]): unknown {
    if (!this.instance) {
      throw new Error("WASM module not loaded. Call load() first.");
    }

    const func = this.instance.exports[functionName];
    if (typeof func !== "function") {
      throw new Error(`Function '${functionName}' not found in WASM exports`);
    }

    return func(...args);
  }

  /**
   * Get the total number of nodes in the embedded graph
   * @returns Number of nodes
   */
  nodeCount(): number {
    return this.call("cov_node_count") as number;
  }

  /**
   * Get a node ID by index
   * @param idx Node index (0 to nodeCount-1)
   * @returns Fat pointer to node ID string (ptr | len)
   */
  getNodeId(idx: number): bigint {
    return this.call("cov_get_node_id", idx) as bigint;
  }

  /**
   * Get node content by index
   * @param idx Node index
   * @returns Fat pointer to content string
   */
  getNodeContent(idx: number): bigint {
    return this.call("cov_get_node_content", idx) as bigint;
  }

  /**
   * Find a node by ID
   * @param id Node ID to search for
   * @returns Node index, or -1 if not found
   */
  findByIdRaw(id: string): number {
    const idBytes = new TextEncoder().encode(id);
    // Allocate memory for the ID string (simplified - assumes heap allocator exists)
    // For now, we'll just use a fixed offset in memory
    // TODO: Proper heap allocation
    const idPtr = 1024 * 1024; // 1MB offset
    if (this.memory) {
      const memBytes = new Uint8Array(this.memory.buffer);
      memBytes.set(idBytes, idPtr);
    }
    return this.call("cov_find_by_id", idPtr, idBytes.length) as number;
  }

  /**
   * Extract a string from WASM memory given a fat pointer
   * @param fatPtr Fat pointer (ptr in upper 32 bits, len in lower 32 bits)
   * @returns Decoded string
   */
  readString(fatPtr: bigint): string {
    if (!this.memory) {
      throw new Error("Memory not available");
    }

    // Fat pointer format: (ptr << 32) | len
    const len = Number(fatPtr & 0xFFFFFFFFn);
    const ptr = Number((fatPtr >> 32n) & 0xFFFFFFFFn);

    if (ptr === 0 || len === 0) {
      return "";
    }

    const bytes = new Uint8Array(this.memory.buffer, ptr, len);
    return new TextDecoder().decode(bytes);
  }

  /**
   * Get all node IDs in the graph
   * @returns Array of node ID strings
   */
  getAllNodeIds(): string[] {
    const count = this.nodeCount();
    const ids: string[] = [];

    for (let i = 0; i < count; i++) {
      const fatPtr = this.getNodeId(i);
      const id = this.readString(fatPtr);
      ids.push(id);
    }

    return ids;
  }

  /**
   * Get all nodes with their IDs and content
   * @returns Array of {id, content} objects
   */
  getAllNodes(): Array<{ id: string; content: string }> {
    const count = this.nodeCount();
    const nodes: Array<{ id: string; content: string }> = [];

    for (let i = 0; i < count; i++) {
      const idPtr = this.getNodeId(i);
      const contentPtr = this.getNodeContent(i);

      nodes.push({
        id: this.readString(idPtr),
        content: this.readString(contentPtr),
      });
    }

    return nodes;
  }

  /**
   * List all exported functions in the WASM module
   * @returns Array of export names
   */
  listExports(): string[] {
    if (!this.instance) {
      throw new Error("WASM module not loaded");
    }

    return Object.keys(this.instance.exports).filter(
      (name) => typeof this.instance!.exports[name] === "function"
    );
  }

  /**
   * Allocate memory in the WASM module using cov_alloc
   * @param size Number of bytes to allocate
   * @returns Pointer to allocated memory
   */
  alloc(size: number): number {
    return this.call("cov_alloc", size) as number;
  }

  /**
   * Allocate and write a string to WASM memory, return fat pointer as BigInt
   * Uses cov_alloc for proper memory allocation.
   * @param str String to allocate
   * @returns Fat pointer: (ptr << 32) | len
   */
  allocString(str: string): bigint {
    if (!this.memory) {
      throw new Error("Memory not available");
    }

    const bytes = new TextEncoder().encode(str);
    const ptr = this.alloc(bytes.length);
    new Uint8Array(this.memory.buffer, ptr, bytes.length).set(bytes);

    // Pack as fat pointer: (ptr << 32) | len
    return (BigInt(ptr) << 32n) | BigInt(bytes.length);
  }

  /**
   * Call a query function with a string parameter
   * Convenience method that allocates the string and calls the function.
   * @param funcName Name of the query function
   * @param searchTerm String parameter to pass
   * @returns Query result (fat pointer to result array)
   */
  queryWithString(funcName: string, searchTerm: string): bigint {
    const fatPtr = this.allocString(searchTerm);
    return this.call(funcName, fatPtr) as bigint;
  }

  /**
   * Extract query results from a fat pointer
   * Query functions return fat pointers to node index arrays.
   * @param resultPtr Fat pointer returned by a query function
   * @returns Array of node indices
   */
  extractQueryResults(resultPtr: bigint): number[] {
    if (!this.memory) {
      throw new Error("Memory not available");
    }

    const ptr = Number(resultPtr >> 32n);
    const count = Number(resultPtr & 0xFFFFFFFFn);

    if (count === 0) return [];

    const view = new DataView(this.memory.buffer, ptr, count * 4);
    const indices: number[] = [];

    for (let i = 0; i < count; i++) {
      indices.push(view.getUint32(i * 4, true)); // little-endian
    }

    return indices;
  }

  /**
   * Get node data for query results
   * @param resultPtr Fat pointer from a query function
   * @returns Array of {idx, id, content, kind} objects
   */
  getQueryResultNodes(
    resultPtr: bigint
  ): Array<{ idx: number; id: string; content: string; kind: string }> {
    const indices = this.extractQueryResults(resultPtr);
    return indices.map((idx) => ({
      idx,
      id: this.readString(this.call("cov_get_node_id", idx) as bigint),
      content: this.readString(this.call("cov_get_node_content", idx) as bigint),
      kind: this.readString(this.call("cov_get_node_kind", idx) as bigint),
    }));
  }
}
