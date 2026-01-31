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
        upper: this.noop.bind(this),
        lower: this.noop.bind(this),
        trim: this.noop.bind(this),
        trim_start: this.noop.bind(this),
        trim_end: this.noop.bind(this),
        str_reverse: this.noop.bind(this),
        str_len: this.noop.bind(this),
        byte_len: this.noop.bind(this),
        is_empty: this.noop.bind(this),
        concat: this.noop.bind(this),
        contains: this.noop.bind(this),
        starts_with: this.noop.bind(this),
        ends_with: this.noop.bind(this),
        index_of: this.noop.bind(this),
        slice: this.noop.bind(this),
        char_at: this.noop.bind(this),
        replace: this.noop.bind(this),
        replace_all: this.noop.bind(this),
        split: this.noop.bind(this),
        join: this.noop.bind(this),
        repeat: this.noop.bind(this),
        pad_start: this.noop.bind(this),
        pad_end: this.noop.bind(this),
      },
      "std.text": {
        regex_test: this.noop.bind(this),
        regex_match: this.noop.bind(this),
        regex_replace: this.noop.bind(this),
        regex_replace_all: this.noop.bind(this),
        regex_split: this.noop.bind(this),
      },
      list: {
        len: this.noop.bind(this),
        is_empty: this.noop.bind(this),
        get: this.noop.bind(this),
        first: this.noop.bind(this),
        last: this.noop.bind(this),
        append: this.noop.bind(this),
        contains: this.noop.bind(this),
        flatten: this.noop.bind(this),
      },
      map: {
        get: this.noop.bind(this),
      },
      fs: {
        read_file: this.noop.bind(this),
        write_file: this.noop.bind(this),
        mkdir: this.noop.bind(this),
        exists: this.noop.bind(this),
        remove: this.noop.bind(this),
        rename: this.noop.bind(this),
        list_dir: this.noop.bind(this),
        read_dir: this.noop.bind(this),
        is_dir: this.noop.bind(this),
        is_file: this.noop.bind(this),
        file_size: this.noop.bind(this),
        stat: this.noop.bind(this),
        copy: this.noop.bind(this),
      },
      path: {
        join: this.noop.bind(this),
        basename: this.noop.bind(this),
        dirname: this.noop.bind(this),
        extname: this.noop.bind(this),
        is_absolute: this.noop.bind(this),
        normalize: this.noop.bind(this),
        resolve: this.noop.bind(this),
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
