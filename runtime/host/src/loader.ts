/**
 * Covenant WASM Module Loader
 *
 * A minimal TypeScript host that loads and links Covenant WASM modules.
 * This implements the runtime architecture where most logic stays in Covenant,
 * and the host only provides module loading, linking, and I/O effects.
 *
 * Architecture:
 * ```
 * ┌─────────────────────────────────────────────────────────────────┐
 * │  TypeScript Host (this file)                                    │
 * │  ┌──────────────────────────────────────────────────────────────┐
 * │  │ Module Loader & Linker                                       │
 * │  │  - Loads WASM modules on demand                              │
 * │  │  - Routes calls between modules                              │
 * │  │  - Provides I/O effects (filesystem, network)                │
 * │  │  - Gates imports based on declared effects (capability system)│
 * │  └──────────────────────────────────────────────────────────────┘
 * │                              ↕                                   │
 * │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
 * │  │  query.wasm     │  │  symbols.wasm   │  │  app.wasm       │  │
 * │  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
 * └─────────────────────────────────────────────────────────────────┘
 * ```
 */

import {
  CapabilityManifest,
  CapabilityEnforcementOptions,
  EmbeddableSymbol,
  DEFAULT_ENFORCEMENT_OPTIONS,
  buildManifestFromSymbols,
  emptyManifest,
  isImportAllowed,
  createDeniedStub,
  parseImportName,
  EFFECT_TO_IMPORTS,
  IMPORT_TO_EFFECT,
} from "./capabilities.ts";

// Re-export capability types for external use
export type {
  CapabilityManifest,
  CapabilityEnforcementOptions,
  EmbeddableSymbol,
} from "./capabilities.ts";

export {
  EFFECT_TO_IMPORTS,
  IMPORT_TO_EFFECT,
} from "./capabilities.ts";

// Type definitions matching the WIT interfaces

export interface Symbol {
  id: string;
  kind: string;
  file: string;
  line: number;
  calls: string[];
  references: string[];
  calledBy: string[];
  referencedBy: string[];
  effects: string[];
  effectClosure: string[];
  requirements: string[];
  tests: string[];
}

export interface SymbolFilter {
  kind?: string;
  hasEffect?: string;
  callsFn?: string;
  calledByFn?: string;
}

export interface QueryRequest {
  selectClause: string;
  fromType: string;
  whereClause?: string;
  orderBy?: string;
  limit?: number;
  offset?: number;
}

export interface QueryResult {
  symbols: Symbol[];
  version: number;
  hasMore: boolean;
}

export interface MutationResult {
  success: boolean;
  errors: string[];
  warnings: string[];
  newVersion: number;
}

export interface CompileResult {
  success: boolean;
  errors: string[];
  wasm?: Uint8Array;
}

// Module interfaces (what each WASM module exports)

interface SymbolsModule {
  getSymbol(id: string): Symbol | null;
  listSymbols(filter: SymbolFilter): Symbol[];
  upsertSymbol(symbol: Symbol): void;
  deleteSymbol(id: string): boolean;
  getVersion(): number;
}

interface QueryModule {
  executeQuery(request: QueryRequest): QueryResult;
  startQuery(request: QueryRequest): number;
  pollQuery(handle: number): "pending" | "complete" | "error" | "cancelled";
  getResult(handle: number): QueryResult | null;
  cancelQuery(handle: number): void;
}

interface MutationModule {
  parseSnippet(source: string): MutationResult;
  updateSnippet(id: string, source: string): MutationResult;
  deleteSnippet(id: string): boolean;
  compileSnippet(id: string): CompileResult;
  recompileSnippet(id: string, source: string): CompileResult;
}

/**
 * Main host class that loads and manages Covenant WASM modules
 */
export class CovenantHost {
  private modules = new Map<string, WebAssembly.Instance>();
  private memory: WebAssembly.Memory | null = null;
  private manifests = new Map<string, CapabilityManifest>();
  private enforcementOptions: CapabilityEnforcementOptions;

  // Module interfaces (populated after loading)
  public symbols: SymbolsModule | null = null;
  public query: QueryModule | null = null;
  public mutation: MutationModule | null = null;

  constructor(options?: Partial<CapabilityEnforcementOptions>) {
    this.enforcementOptions = { ...DEFAULT_ENFORCEMENT_OPTIONS, ...options };
  }

  /**
   * Load a WASM module from a file path or URL
   */
  async loadModule(name: string, wasmPath: string): Promise<void> {
    const wasmBuffer = await this.fetchWasm(wasmPath);

    // First, compile to inspect the module structure
    const wasmModule = await WebAssembly.compile(wasmBuffer);

    // Build minimal imports to instantiate and extract capabilities
    const minimalImports = this.buildMinimalImports(name);
    // When instantiating a compiled Module, it returns an Instance directly
    const probeInstance = await WebAssembly.instantiate(
      wasmModule,
      minimalImports
    );

    // Extract capability manifest from embedded metadata
    const manifest = this.extractCapabilities(probeInstance);
    this.manifests.set(name, manifest);

    // Now build filtered imports based on the manifest
    const filteredImports = this.buildFilteredImports(name, manifest);

    // Instantiate with filtered imports
    const instance = await WebAssembly.instantiate(
      wasmModule,
      filteredImports
    );
    this.modules.set(name, instance);

    // Initialize if the module has an _initialize export
    const init = instance.exports._initialize as (() => void) | undefined;
    if (init) {
      init();
    }

    // Update module interfaces
    this.updateModuleInterfaces();
  }

  /**
   * Get the capability manifest for a loaded module
   */
  getManifest(moduleName: string): CapabilityManifest | undefined {
    return this.manifests.get(moduleName);
  }

  /**
   * Initialize the runtime by loading core modules in dependency order
   */
  async init(wasmDir: string = "."): Promise<void> {
    // Load modules in dependency order
    await this.loadModule("symbols", `${wasmDir}/covenant-symbols.wasm`);
    await this.loadModule("query", `${wasmDir}/covenant-query.wasm`);
    await this.loadModule("mutation", `${wasmDir}/covenant-mutation.wasm`);
  }

  /**
   * Hot-swap a module with a new version
   */
  async swapModule(name: string, newWasmPath: string): Promise<void> {
    // Load the new module
    const tempName = `${name}_new`;
    await this.loadModule(tempName, newWasmPath);

    // Swap pointers atomically
    const newModule = this.modules.get(tempName);
    if (newModule) {
      this.modules.set(name, newModule);
      this.modules.delete(tempName);
    }

    // Update interfaces
    this.updateModuleInterfaces();
  }

  /**
   * Execute a query against the symbol store
   */
  executeQuery(request: QueryRequest): QueryResult {
    if (!this.query) {
      throw new Error("Query module not loaded");
    }
    return this.query.executeQuery(request);
  }

  /**
   * Update a snippet and trigger recompilation
   */
  async reloadSnippet(
    snippetId: string,
    newSource: string
  ): Promise<MutationResult> {
    if (!this.mutation) {
      throw new Error("Mutation module not loaded");
    }

    // 1. Parse and validate
    const parseResult = this.mutation.parseSnippet(newSource);
    if (!parseResult.success) {
      return parseResult;
    }

    // 2. Update symbol store
    const updateResult = this.mutation.updateSnippet(snippetId, newSource);
    if (!updateResult.success) {
      return updateResult;
    }

    // 3. Compile to WASM
    const compileResult = this.mutation.compileSnippet(snippetId);
    if (!compileResult.success) {
      return {
        success: false,
        errors: compileResult.errors,
        warnings: [],
        newVersion: updateResult.newVersion,
      };
    }

    // 4. Hot swap the compiled module (if we have a WASM binary)
    if (compileResult.wasm) {
      // In a real implementation, we would load this as a new module
      // For now, just log success
      console.log(`Snippet ${snippetId} recompiled (${compileResult.wasm.length} bytes)`);
    }

    return updateResult;
  }

  // === Private Methods ===

  private async fetchWasm(path: string): Promise<ArrayBuffer> {
    // Deno environment (preferred)
    if (typeof (globalThis as any).Deno !== "undefined") {
      const buffer = await (globalThis as any).Deno.readFile(path);
      return buffer.buffer;
    }

    // Browser environment
    if (typeof fetch !== "undefined" && typeof (globalThis as any).Deno === "undefined") {
      const response = await fetch(path);
      return response.arrayBuffer();
    }

    // Node.js environment
    if (typeof require !== "undefined") {
      const fs = require("fs");
      const buffer = fs.readFileSync(path);
      return buffer.buffer.slice(
        buffer.byteOffset,
        buffer.byteOffset + buffer.byteLength
      );
    }

    throw new Error("No method available to load WASM file");
  }

  /**
   * Build minimal imports for probing module capabilities.
   * All effect-gated imports are stubs that won't actually be called.
   */
  private buildMinimalImports(moduleName: string): WebAssembly.Imports {
    return {
      // Link to other Covenant modules (if already loaded)
      "covenant:runtime/symbols": this.buildSymbolsImport(),
      "covenant:runtime/query": this.buildQueryImport(),

      // Stub all host-provided I/O effects for probing
      "wasi:filesystem/types": this.buildFilesystemStubs(),
      "wasi:http/outgoing-handler": this.buildHttpStubs(),

      // Effect-gated imports (all stubs for probing)
      db: this.buildDatabaseStubs(),
      http: this.buildNetworkStubs(),
      fs: this.buildFilesystemEffectStubs(),
      console: this.buildConsoleStubs(),
      "std.storage.kv": this.buildStorageKvStubs(),
      "std.storage.doc": this.buildStorageDocStubs(),
      "std.time": this.buildTimeStubs(),
      "std.random": this.buildRandomStubs(),
      "std.crypto": this.buildCryptoStubs(),
      mem: this.buildMemStubs(),
      text: this.buildTextStubs(),
      list: this.buildListStubs(),
      path: this.buildPathStubs(),

      // Environment
      env: {
        memory: this.getOrCreateMemory(),
        log: () => {},
        now: () => Date.now(),
      },
    };
  }

  /**
   * Build filtered imports based on the capability manifest.
   * Only provides real implementations for allowed capabilities;
   * denied capabilities get stubs that throw errors.
   */
  private buildFilteredImports(
    moduleName: string,
    manifest: CapabilityManifest
  ): WebAssembly.Imports {
    return {
      // Link to other Covenant modules (always allowed)
      "covenant:runtime/symbols": this.buildSymbolsImport(),
      "covenant:runtime/query": this.buildQueryImport(),

      // Host-provided I/O effects (filtered)
      "wasi:filesystem/types": this.filterImportObject(
        "wasi:filesystem/types",
        this.buildFilesystemImport(),
        manifest
      ),
      "wasi:http/outgoing-handler": this.filterImportObject(
        "wasi:http/outgoing-handler",
        this.buildHttpImport(),
        manifest
      ),

      // Effect-gated imports (filtered based on manifest)
      db: this.filterImportObject(
        "db",
        this.buildDatabaseImport(),
        manifest
      ),
      http: this.filterImportObject(
        "http",
        this.buildNetworkImport(),
        manifest
      ),
      fs: this.filterImportObject(
        "fs",
        this.buildFilesystemEffectImport(),
        manifest
      ),
      console: this.filterImportObject(
        "console",
        this.buildConsoleImport(moduleName),
        manifest
      ),
      "std.storage.kv": this.filterImportObject(
        "std.storage.kv",
        this.buildStorageKvImport(),
        manifest
      ),
      "std.storage.doc": this.filterImportObject(
        "std.storage.doc",
        this.buildStorageDocImport(),
        manifest
      ),
      "std.time": this.filterImportObject(
        "std.time",
        this.buildTimeImport(),
        manifest
      ),
      "std.random": this.filterImportObject(
        "std.random",
        this.buildRandomImport(),
        manifest
      ),
      "std.crypto": this.filterImportObject(
        "std.crypto",
        this.buildCryptoImport(),
        manifest
      ),
      mem: this.buildMemImport(), // Always allowed (internal)
      text: this.buildTextImport(), // Pure operations, always allowed
      list: this.buildListImport(), // Pure operations, always allowed
      path: this.buildPathImport(), // Pure operations, always allowed

      // Environment
      env: {
        memory: this.getOrCreateMemory(),
        log: (ptr: number, len: number) => {
          const str = this.readString(ptr, len);
          console.log(`[${moduleName}]`, str);
        },
        now: () => Date.now(),
      },
    };
  }

  /**
   * Filter an import object based on the capability manifest.
   * Returns real implementations for allowed imports, stubs for denied ones.
   */
  private filterImportObject(
    modulePrefix: string,
    realImports: Record<string, Function>,
    manifest: CapabilityManifest
  ): Record<string, Function> {
    const filtered: Record<string, Function> = {};

    for (const [funcName, impl] of Object.entries(realImports)) {
      const importName = `${modulePrefix}.${funcName}`;

      if (isImportAllowed(manifest, importName)) {
        filtered[funcName] = impl;
      } else if (this.enforcementOptions.strict) {
        if (this.enforcementOptions.warnOnly) {
          filtered[funcName] = (...args: unknown[]) => {
            console.warn(`[capability] Denied import called: ${importName}`);
            this.enforcementOptions.onDenied?.(importName);
            return 0;
          };
        } else {
          filtered[funcName] = createDeniedStub(importName);
        }
      } else {
        // Non-strict mode: provide the real implementation
        filtered[funcName] = impl;
      }
    }

    return filtered;
  }

  /**
   * Extract capability manifest from a WASM instance's embedded metadata.
   *
   * Looks for the _cov_get_symbol_metadata export which returns a fat pointer
   * (i64) encoding: (offset << 32) | length
   */
  private extractCapabilities(instance: WebAssembly.Instance): CapabilityManifest {
    const getMetadata = instance.exports._cov_get_symbol_metadata as
      | (() => bigint)
      | undefined;

    if (!getMetadata) {
      // Module doesn't have embedded metadata - return empty manifest
      return emptyManifest();
    }

    const memory = instance.exports.memory as WebAssembly.Memory | undefined;
    if (!memory) {
      return emptyManifest();
    }

    try {
      // Call the metadata function to get the fat pointer
      const fatPtr = getMetadata();

      // Unpack fat pointer: (offset << 32) | length
      const offset = Number(fatPtr >> BigInt(32));
      const length = Number(fatPtr & BigInt(0xffffffff));

      if (length === 0) {
        return emptyManifest();
      }

      // Read JSON from memory
      const bytes = new Uint8Array(memory.buffer, offset, length);
      const json = new TextDecoder().decode(bytes);
      const symbols: EmbeddableSymbol[] = JSON.parse(json);

      return buildManifestFromSymbols(symbols);
    } catch (error) {
      console.warn("[capability] Failed to extract capabilities:", error);
      return emptyManifest();
    }
  }

  // Legacy buildImports for backwards compatibility
  private buildImports(moduleName: string): WebAssembly.Imports {
    return {
      // Link to other Covenant modules (if already loaded)
      "covenant:runtime/symbols": this.buildSymbolsImport(),
      "covenant:runtime/query": this.buildQueryImport(),

      // Host-provided I/O effects
      "wasi:filesystem/types": this.buildFilesystemImport(),
      "wasi:http/outgoing-handler": this.buildHttpImport(),

      // Environment
      env: {
        memory: this.getOrCreateMemory(),
        // Logging
        log: (ptr: number, len: number) => {
          const str = this.readString(ptr, len);
          console.log(`[${moduleName}]`, str);
        },
        // Current time
        now: () => Date.now(),
      },
    };
  }

  private buildSymbolsImport(): Record<string, Function> {
    const symbolsModule = this.modules.get("symbols");
    if (!symbolsModule) {
      // Return stubs if not yet loaded
      return {
        getSymbol: () => null,
        listSymbols: () => [],
        upsertSymbol: () => {},
        deleteSymbol: () => false,
        getVersion: () => 0,
      };
    }
    return symbolsModule.exports as Record<string, Function>;
  }

  private buildQueryImport(): Record<string, Function> {
    const queryModule = this.modules.get("query");
    if (!queryModule) {
      return {
        executeQuery: () => ({ symbols: [], version: 0, hasMore: false }),
        startQuery: () => 0,
        pollQuery: () => "error",
        getResult: () => null,
        cancelQuery: () => {},
      };
    }
    return queryModule.exports as Record<string, Function>;
  }

  private buildFilesystemImport(): Record<string, Function> {
    // Stub implementation - real implementation would use Node.js fs or browser APIs
    return {
      readFile: (pathPtr: number, pathLen: number) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] readFile: ${path}`);
        return null;
      },
      writeFile: (
        pathPtr: number,
        pathLen: number,
        dataPtr: number,
        dataLen: number
      ) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] writeFile: ${path} (${dataLen} bytes)`);
        return true;
      },
    };
  }

  private buildHttpImport(): Record<string, Function> {
    // Stub implementation - real implementation would use fetch
    return {
      fetch: (urlPtr: number, urlLen: number) => {
        const url = this.readString(urlPtr, urlLen);
        console.log(`[http] fetch: ${url}`);
        return 0; // Handle
      },
    };
  }

  // === Stub builders for minimal imports (probing) ===

  private buildFilesystemStubs(): Record<string, Function> {
    return {
      readFile: () => null,
      writeFile: () => true,
    };
  }

  private buildHttpStubs(): Record<string, Function> {
    return {
      fetch: () => 0,
    };
  }

  private buildDatabaseStubs(): Record<string, Function> {
    return {
      execute_query: () => 0,
    };
  }

  private buildNetworkStubs(): Record<string, Function> {
    return {
      fetch: () => 0,
    };
  }

  private buildFilesystemEffectStubs(): Record<string, Function> {
    return {
      read: () => 0,
      write: () => 0,
      delete: () => 0,
      exists: () => 0,
      read_dir: () => 0,
      create_dir: () => 0,
      remove_dir: () => 0,
    };
  }

  private buildConsoleStubs(): Record<string, Function> {
    return {
      println: () => {},
      print: () => {},
      eprintln: () => {},
      eprint: () => {},
    };
  }

  private buildStorageKvStubs(): Record<string, Function> {
    return {
      get: () => 0,
      set: () => {},
      delete: () => {},
      has: () => 0,
      list: () => 0,
      clear: () => {},
    };
  }

  private buildStorageDocStubs(): Record<string, Function> {
    return {
      put: () => 0,
      get: () => 0,
      delete: () => {},
      query: () => 0,
      count: () => 0,
      create_index: () => {},
    };
  }

  private buildTimeStubs(): Record<string, Function> {
    return {
      now: () => 0,
      sleep: () => {},
    };
  }

  private buildRandomStubs(): Record<string, Function> {
    return {
      int: () => 0,
      float: () => 0,
      bytes: () => 0,
    };
  }

  private buildCryptoStubs(): Record<string, Function> {
    return {
      hash: () => 0,
      sign: () => 0,
      verify: () => 0,
    };
  }

  private buildMemStubs(): Record<string, Function> {
    return {
      alloc: () => 0,
    };
  }

  private buildTextStubs(): Record<string, Function> {
    return {
      concat: () => 0,
      length: () => 0,
      substring: () => 0,
      to_uppercase: () => 0,
      to_lowercase: () => 0,
      trim: () => 0,
      split: () => 0,
      join: () => 0,
      contains: () => 0,
      starts_with: () => 0,
      ends_with: () => 0,
      replace: () => 0,
      regex_test: () => 0,
      regex_match: () => 0,
      regex_replace: () => 0,
    };
  }

  private buildListStubs(): Record<string, Function> {
    return {
      length: () => 0,
      get: () => 0,
      push: () => 0,
      pop: () => 0,
      slice: () => 0,
      concat: () => 0,
      map: () => 0,
      filter: () => 0,
      reduce: () => 0,
      find: () => 0,
      some: () => 0,
      every: () => 0,
      reverse: () => 0,
      sort: () => 0,
    };
  }

  private buildPathStubs(): Record<string, Function> {
    return {
      join: () => 0,
      dirname: () => 0,
      basename: () => 0,
      extname: () => 0,
      normalize: () => 0,
      is_absolute: () => 0,
    };
  }

  // === Real import builders for filtered imports ===

  private buildDatabaseImport(): Record<string, Function> {
    return {
      execute_query: (sqlPtr: number, sqlLen: number, paramCount: number) => {
        const sql = this.readString(sqlPtr, sqlLen);
        console.log(`[db] execute_query: ${sql} (${paramCount} params)`);
        // TODO: Implement actual database execution
        return 0;
      },
    };
  }

  private buildNetworkImport(): Record<string, Function> {
    return {
      fetch: async (urlPtr: number, urlLen: number) => {
        const url = this.readString(urlPtr, urlLen);
        console.log(`[http] fetch: ${url}`);
        // TODO: Implement actual fetch
        return 0;
      },
    };
  }

  private buildFilesystemEffectImport(): Record<string, Function> {
    return {
      read: (pathPtr: number, pathLen: number) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] read: ${path}`);
        // TODO: Implement actual file read
        return 0;
      },
      write: (pathPtr: number, pathLen: number, dataPtr: number, dataLen: number) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] write: ${path} (${dataLen} bytes)`);
        // TODO: Implement actual file write
        return 0;
      },
      delete: (pathPtr: number, pathLen: number) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] delete: ${path}`);
        // TODO: Implement actual file delete
        return 0;
      },
      exists: (pathPtr: number, pathLen: number) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] exists: ${path}`);
        // TODO: Implement actual exists check
        return 0;
      },
      read_dir: (pathPtr: number, pathLen: number) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] read_dir: ${path}`);
        // TODO: Implement actual directory read
        return 0;
      },
      create_dir: (pathPtr: number, pathLen: number) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] create_dir: ${path}`);
        // TODO: Implement actual directory creation
        return 0;
      },
      remove_dir: (pathPtr: number, pathLen: number) => {
        const path = this.readString(pathPtr, pathLen);
        console.log(`[fs] remove_dir: ${path}`);
        // TODO: Implement actual directory removal
        return 0;
      },
    };
  }

  private buildConsoleImport(moduleName: string): Record<string, Function> {
    return {
      println: (ptr: number, len: number) => {
        const str = this.readString(ptr, len);
        console.log(`[${moduleName}]`, str);
      },
      print: (ptr: number, len: number) => {
        const str = this.readString(ptr, len);
        // Use console.log for cross-platform compatibility
        // (stdout.write without newline is not portable)
        console.log(`[${moduleName}]`, str);
      },
      eprintln: (ptr: number, len: number) => {
        const str = this.readString(ptr, len);
        console.error(`[${moduleName}]`, str);
      },
      eprint: (ptr: number, len: number) => {
        const str = this.readString(ptr, len);
        // Use console.error for cross-platform compatibility
        console.error(`[${moduleName}]`, str);
      },
    };
  }

  private buildStorageKvImport(): Record<string, Function> {
    // TODO: Implement actual key-value storage
    const store = new Map<string, string>();
    return {
      get: (keyPtr: number, keyLen: number) => {
        const key = this.readString(keyPtr, keyLen);
        console.log(`[std.storage.kv] get: ${key}`);
        return store.has(key) ? 1 : 0;
      },
      set: (keyPtr: number, keyLen: number, valuePtr: number, valueLen: number) => {
        const key = this.readString(keyPtr, keyLen);
        const value = this.readString(valuePtr, valueLen);
        console.log(`[std.storage.kv] set: ${key} = ${value}`);
        store.set(key, value);
      },
      delete: (keyPtr: number, keyLen: number) => {
        const key = this.readString(keyPtr, keyLen);
        console.log(`[std.storage.kv] delete: ${key}`);
        store.delete(key);
      },
      has: (keyPtr: number, keyLen: number) => {
        const key = this.readString(keyPtr, keyLen);
        return store.has(key) ? 1 : 0;
      },
      list: (prefixPtr: number, prefixLen: number) => {
        const prefix = this.readString(prefixPtr, prefixLen);
        console.log(`[std.storage.kv] list: ${prefix}`);
        return 0;
      },
      clear: (prefixPtr: number, prefixLen: number) => {
        const prefix = this.readString(prefixPtr, prefixLen);
        console.log(`[std.storage.kv] clear: ${prefix}`);
        for (const key of store.keys()) {
          if (key.startsWith(prefix)) {
            store.delete(key);
          }
        }
      },
    };
  }

  private buildStorageDocImport(): Record<string, Function> {
    // TODO: Implement actual document storage
    return {
      put: (collPtr: number, collLen: number, idPtr: number, idLen: number, dataPtr: number, dataLen: number) => {
        const collection = this.readString(collPtr, collLen);
        const id = this.readString(idPtr, idLen);
        console.log(`[std.storage.doc] put: ${collection}/${id}`);
        return 0;
      },
      get: (collPtr: number, collLen: number, idPtr: number, idLen: number) => {
        const collection = this.readString(collPtr, collLen);
        const id = this.readString(idPtr, idLen);
        console.log(`[std.storage.doc] get: ${collection}/${id}`);
        return 0;
      },
      delete: (collPtr: number, collLen: number, idPtr: number, idLen: number) => {
        const collection = this.readString(collPtr, collLen);
        const id = this.readString(idPtr, idLen);
        console.log(`[std.storage.doc] delete: ${collection}/${id}`);
      },
      query: (collPtr: number, collLen: number, filterPtr: number, filterLen: number) => {
        const collection = this.readString(collPtr, collLen);
        console.log(`[std.storage.doc] query: ${collection}`);
        return 0;
      },
      count: (collPtr: number, collLen: number, filterPtr: number, filterLen: number) => {
        const collection = this.readString(collPtr, collLen);
        console.log(`[std.storage.doc] count: ${collection}`);
        return 0;
      },
      create_index: (collPtr: number, collLen: number, fieldPtr: number, fieldLen: number) => {
        const collection = this.readString(collPtr, collLen);
        const field = this.readString(fieldPtr, fieldLen);
        console.log(`[std.storage.doc] create_index: ${collection}.${field}`);
      },
    };
  }

  private buildTimeImport(): Record<string, Function> {
    return {
      now: () => BigInt(Date.now()),
      sleep: (ms: number) => {
        console.log(`[std.time] sleep: ${ms}ms`);
        // Note: Synchronous sleep is not ideal but matches WASM expectations
        const end = Date.now() + ms;
        while (Date.now() < end) {
          // Busy wait
        }
      },
    };
  }

  private buildRandomImport(): Record<string, Function> {
    return {
      int: (min: number, max: number) => {
        return Math.floor(Math.random() * (max - min + 1)) + min;
      },
      float: () => Math.random(),
      bytes: (len: number) => {
        // TODO: Return pointer to allocated random bytes
        console.log(`[std.random] bytes: ${len}`);
        return 0;
      },
    };
  }

  private buildCryptoImport(): Record<string, Function> {
    // TODO: Implement actual crypto operations
    return {
      hash: (algPtr: number, algLen: number, dataPtr: number, dataLen: number) => {
        const alg = this.readString(algPtr, algLen);
        console.log(`[std.crypto] hash: ${alg}`);
        return 0;
      },
      sign: (keyPtr: number, keyLen: number, dataPtr: number, dataLen: number) => {
        console.log(`[std.crypto] sign`);
        return 0;
      },
      verify: (keyPtr: number, keyLen: number, dataPtr: number, dataLen: number, sigPtr: number, sigLen: number) => {
        console.log(`[std.crypto] verify`);
        return 0;
      },
    };
  }

  private buildMemImport(): Record<string, Function> {
    return {
      alloc: (size: number) => {
        // Simple bump allocator using global heap pointer
        // In practice, this would need proper memory management
        console.log(`[mem] alloc: ${size} bytes`);
        return 0; // TODO: Implement actual allocation
      },
    };
  }

  private buildTextImport(): Record<string, Function> {
    // Pure text operations - always allowed
    return {
      concat: (aPtr: number, aLen: number, bPtr: number, bLen: number) => {
        const a = this.readString(aPtr, aLen);
        const b = this.readString(bPtr, bLen);
        console.log(`[text] concat: "${a}" + "${b}"`);
        return 0; // TODO: Return allocated string
      },
      length: (ptr: number, len: number) => {
        return len;
      },
      substring: (ptr: number, len: number, start: number, end: number) => {
        const s = this.readString(ptr, len);
        console.log(`[text] substring: "${s}"[${start}:${end}]`);
        return 0;
      },
      to_uppercase: (ptr: number, len: number) => {
        const s = this.readString(ptr, len);
        console.log(`[text] to_uppercase: "${s}"`);
        return 0;
      },
      to_lowercase: (ptr: number, len: number) => {
        const s = this.readString(ptr, len);
        console.log(`[text] to_lowercase: "${s}"`);
        return 0;
      },
      trim: (ptr: number, len: number) => {
        const s = this.readString(ptr, len);
        console.log(`[text] trim: "${s}"`);
        return 0;
      },
      split: (ptr: number, len: number, delimPtr: number, delimLen: number) => {
        const s = this.readString(ptr, len);
        const delim = this.readString(delimPtr, delimLen);
        console.log(`[text] split: "${s}" by "${delim}"`);
        return 0;
      },
      join: (listPtr: number, listLen: number, delimPtr: number, delimLen: number) => {
        console.log(`[text] join`);
        return 0;
      },
      contains: (ptr: number, len: number, needlePtr: number, needleLen: number) => {
        const s = this.readString(ptr, len);
        const needle = this.readString(needlePtr, needleLen);
        return s.includes(needle) ? 1 : 0;
      },
      starts_with: (ptr: number, len: number, prefixPtr: number, prefixLen: number) => {
        const s = this.readString(ptr, len);
        const prefix = this.readString(prefixPtr, prefixLen);
        return s.startsWith(prefix) ? 1 : 0;
      },
      ends_with: (ptr: number, len: number, suffixPtr: number, suffixLen: number) => {
        const s = this.readString(ptr, len);
        const suffix = this.readString(suffixPtr, suffixLen);
        return s.endsWith(suffix) ? 1 : 0;
      },
      replace: (ptr: number, len: number, fromPtr: number, fromLen: number, toPtr: number, toLen: number) => {
        const s = this.readString(ptr, len);
        const from = this.readString(fromPtr, fromLen);
        const to = this.readString(toPtr, toLen);
        console.log(`[text] replace: "${s}" "${from}" -> "${to}"`);
        return 0;
      },
      regex_test: (ptr: number, len: number, patternPtr: number, patternLen: number) => {
        const s = this.readString(ptr, len);
        const pattern = this.readString(patternPtr, patternLen);
        try {
          const re = new RegExp(pattern);
          return re.test(s) ? 1 : 0;
        } catch {
          return 0;
        }
      },
      regex_match: (ptr: number, len: number, patternPtr: number, patternLen: number) => {
        console.log(`[text] regex_match`);
        return 0;
      },
      regex_replace: (ptr: number, len: number, patternPtr: number, patternLen: number, replPtr: number, replLen: number) => {
        console.log(`[text] regex_replace`);
        return 0;
      },
    };
  }

  private buildListImport(): Record<string, Function> {
    // Pure list operations - always allowed
    return {
      length: (ptr: number, len: number) => len,
      get: (ptr: number, len: number, index: number) => 0,
      push: (ptr: number, len: number, elemPtr: number, elemLen: number) => 0,
      pop: (ptr: number, len: number) => 0,
      slice: (ptr: number, len: number, start: number, end: number) => 0,
      concat: (aPtr: number, aLen: number, bPtr: number, bLen: number) => 0,
      map: (ptr: number, len: number, fnIdx: number) => 0,
      filter: (ptr: number, len: number, fnIdx: number) => 0,
      reduce: (ptr: number, len: number, fnIdx: number, initPtr: number, initLen: number) => 0,
      find: (ptr: number, len: number, fnIdx: number) => 0,
      some: (ptr: number, len: number, fnIdx: number) => 0,
      every: (ptr: number, len: number, fnIdx: number) => 0,
      reverse: (ptr: number, len: number) => 0,
      sort: (ptr: number, len: number, fnIdx: number) => 0,
    };
  }

  private buildPathImport(): Record<string, Function> {
    // Pure path operations - always allowed
    return {
      join: (aPtr: number, aLen: number, bPtr: number, bLen: number) => {
        const a = this.readString(aPtr, aLen);
        const b = this.readString(bPtr, bLen);
        console.log(`[path] join: "${a}" + "${b}"`);
        return 0;
      },
      dirname: (ptr: number, len: number) => {
        const p = this.readString(ptr, len);
        console.log(`[path] dirname: "${p}"`);
        return 0;
      },
      basename: (ptr: number, len: number) => {
        const p = this.readString(ptr, len);
        console.log(`[path] basename: "${p}"`);
        return 0;
      },
      extname: (ptr: number, len: number) => {
        const p = this.readString(ptr, len);
        console.log(`[path] extname: "${p}"`);
        return 0;
      },
      normalize: (ptr: number, len: number) => {
        const p = this.readString(ptr, len);
        console.log(`[path] normalize: "${p}"`);
        return 0;
      },
      is_absolute: (ptr: number, len: number) => {
        const p = this.readString(ptr, len);
        return p.startsWith("/") || /^[A-Za-z]:/.test(p) ? 1 : 0;
      },
    };
  }

  private getOrCreateMemory(): WebAssembly.Memory {
    if (!this.memory) {
      this.memory = new WebAssembly.Memory({ initial: 1, maximum: 256 });
    }
    return this.memory;
  }

  private readString(ptr: number, len: number): string {
    if (!this.memory) {
      return "";
    }
    const bytes = new Uint8Array(this.memory.buffer, ptr, len);
    return new TextDecoder().decode(bytes);
  }

  private updateModuleInterfaces(): void {
    // Wrap raw exports in typed interfaces
    const symbolsModule = this.modules.get("symbols");
    if (symbolsModule) {
      this.symbols = symbolsModule.exports as unknown as SymbolsModule;
    }

    const queryModule = this.modules.get("query");
    if (queryModule) {
      this.query = queryModule.exports as unknown as QueryModule;
    }

    const mutationModule = this.modules.get("mutation");
    if (mutationModule) {
      this.mutation = mutationModule.exports as unknown as MutationModule;
    }
  }
}

/**
 * Helper class for hot-reloading individual snippets
 */
export class SnippetReloader {
  constructor(private host: CovenantHost) {}

  /**
   * Reload a single snippet with new source code
   */
  async reload(snippetId: string, newSource: string): Promise<MutationResult> {
    return this.host.reloadSnippet(snippetId, newSource);
  }

  /**
   * Validate a snippet without applying changes
   */
  validate(source: string): MutationResult {
    if (!this.host.mutation) {
      return {
        success: false,
        errors: ["Mutation module not loaded"],
        warnings: [],
        newVersion: 0,
      };
    }
    return this.host.mutation.parseSnippet(source);
  }
}

// Example usage (when running as a script)
async function main() {
  const host = new CovenantHost();

  try {
    // Initialize the runtime
    await host.init("./wasm");

    // Execute a query
    const result = host.executeQuery({
      selectClause: "all",
      fromType: "functions",
      whereClause: JSON.stringify({ hasEffect: "database" }),
    });

    console.log(`Found ${result.symbols.length} database functions`);
    console.log(`Symbol graph version: ${result.version}`);

    // Hot reload a snippet
    const reloader = new SnippetReloader(host);
    const reloadResult = await reloader.reload(
      "auth.login",
      `
      snippet id="auth.login" kind="fn"
      effects
        effect database
        effect network
      end
      end
    `
    );

    if (reloadResult.success) {
      console.log(`Reloaded auth.login (version ${reloadResult.newVersion})`);
    } else {
      console.error("Reload failed:", reloadResult.errors);
    }
  } catch (error) {
    console.error("Error:", error);
  }
}

// Run if executed directly
if (typeof require !== "undefined" && require.main === module) {
  main();
}
