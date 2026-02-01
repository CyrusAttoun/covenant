/**
 * Capability Manifest and Effect-to-Import Mapping
 *
 * This module defines the runtime capability system that gates WASM imports
 * based on declared effects. When a Covenant module is instantiated, the host
 * extracts its capability manifest from the embedded metadata and only provides
 * imports for capabilities that are explicitly declared.
 *
 * Architecture:
 * ```
 * Compile-time:                     Runtime:
 * effect_closure ──────────────────► required_capabilities (in WASM data section)
 *                                           │
 *                                           ▼
 *                                   CapabilityManifest
 *                                           │
 *                                           ▼
 *                               buildFilteredImports()
 *                                           │
 *                                   ┌───────┴───────┐
 *                                   ▼               ▼
 *                            Provided           Denied
 *                            (real impl)       (throws error)
 * ```
 */

/**
 * EmbeddableSymbol structure as serialized in the WASM data section.
 * This mirrors the Rust EmbeddableSymbol struct.
 */
export interface EmbeddableSymbol {
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
  required_capabilities: string[];
}

/**
 * Capability manifest extracted from a WASM module's embedded metadata.
 *
 * Contains the declared effects and derived import requirements for
 * runtime enforcement of capability restrictions.
 */
export interface CapabilityManifest {
  /** All effects declared across all symbols in the module */
  declaredEffects: Set<string>;

  /** Required WASM imports derived from the effect closure */
  requiredImports: Set<string>;

  /** Module symbols for debugging/introspection */
  symbols: EmbeddableSymbol[];
}

/**
 * Canonical mapping from Covenant effects to WASM import names.
 *
 * This must be kept in sync with the Rust effects_to_capabilities function
 * in crates/covenant-codegen/src/embeddable.rs
 */
export const EFFECT_TO_IMPORTS: Record<string, string[]> = {
  // Core effects
  database: ["db.execute_query"],
  network: ["http.fetch"],
  filesystem: [
    "fs.read",
    "fs.write",
    "fs.delete",
    "fs.exists",
    "fs.read_dir",
    "fs.create_dir",
    "fs.remove_dir",
  ],
  console: [
    "console.println",
    "console.print",
    "console.eprintln",
    "console.eprint",
  ],

  // Standard library effects
  "std.storage": [
    "std.storage.kv.get",
    "std.storage.kv.set",
    "std.storage.kv.delete",
    "std.storage.kv.has",
    "std.storage.kv.list",
    "std.storage.kv.clear",
    "std.storage.doc.put",
    "std.storage.doc.get",
    "std.storage.doc.delete",
    "std.storage.doc.query",
    "std.storage.doc.count",
    "std.storage.doc.create_index",
  ],
  "std.time": ["std.time.now", "std.time.sleep"],
  "std.random": ["std.random.int", "std.random.float", "std.random.bytes"],
  "std.crypto": ["std.crypto.hash", "std.crypto.sign", "std.crypto.verify"],
};

/**
 * Reverse mapping from WASM import to the effect that grants it.
 * Built from EFFECT_TO_IMPORTS for quick lookup.
 */
export const IMPORT_TO_EFFECT: Record<string, string> = Object.entries(
  EFFECT_TO_IMPORTS
).reduce(
  (acc, [effect, imports]) => {
    for (const imp of imports) {
      acc[imp] = effect;
    }
    return acc;
  },
  {} as Record<string, string>
);

/**
 * Parse the WASM import name into module and function parts.
 * E.g., "console.println" -> { module: "console", func: "println" }
 */
export function parseImportName(importName: string): {
  module: string;
  func: string;
} {
  const lastDot = importName.lastIndexOf(".");
  if (lastDot === -1) {
    return { module: "env", func: importName };
  }
  return {
    module: importName.substring(0, lastDot),
    func: importName.substring(lastDot + 1),
  };
}

/**
 * Create an empty capability manifest (for modules without metadata).
 */
export function emptyManifest(): CapabilityManifest {
  return {
    declaredEffects: new Set(),
    requiredImports: new Set(),
    symbols: [],
  };
}

/**
 * Build a capability manifest from embedded symbol metadata.
 *
 * @param symbols - Array of EmbeddableSymbol parsed from WASM data section
 * @returns CapabilityManifest with aggregated effects and imports
 */
export function buildManifestFromSymbols(
  symbols: EmbeddableSymbol[]
): CapabilityManifest {
  const declaredEffects = new Set<string>();
  const requiredImports = new Set<string>();

  for (const symbol of symbols) {
    // Collect declared effects
    for (const effect of symbol.effects) {
      declaredEffects.add(effect);
    }

    // Collect required capabilities (derived from effect_closure at compile time)
    for (const cap of symbol.required_capabilities) {
      requiredImports.add(cap);
    }
  }

  return {
    declaredEffects,
    requiredImports,
    symbols,
  };
}

/**
 * Check if a specific import is allowed by the capability manifest.
 *
 * @param manifest - The module's capability manifest
 * @param importName - The WASM import name (e.g., "console.println")
 * @returns true if the import is allowed
 */
export function isImportAllowed(
  manifest: CapabilityManifest,
  importName: string
): boolean {
  // If the manifest has no requirements, allow all (backwards compatibility)
  if (manifest.requiredImports.size === 0) {
    return true;
  }

  return manifest.requiredImports.has(importName);
}

/**
 * Get a human-readable error message for a denied import.
 */
export function getDeniedImportError(importName: string): string {
  const effect = IMPORT_TO_EFFECT[importName];
  if (effect) {
    return `Import '${importName}' denied: requires effect '${effect}' which is not declared`;
  }
  return `Import '${importName}' denied: not in module's required capabilities`;
}

/**
 * Create a stub function that throws an error when called.
 * Used for imports that are not allowed by the capability manifest.
 */
export function createDeniedStub(importName: string): (...args: unknown[]) => never {
  return (..._args: unknown[]): never => {
    throw new Error(getDeniedImportError(importName));
  };
}

/**
 * Options for capability enforcement.
 */
export interface CapabilityEnforcementOptions {
  /** If true, deny all imports not explicitly allowed. Default: true */
  strict: boolean;

  /** If true, log warnings for denied imports instead of throwing. Default: false */
  warnOnly: boolean;

  /** Custom handler for denied imports. If provided, overrides default behavior. */
  onDenied?: (importName: string) => void;
}

/**
 * Default enforcement options: strict mode enabled.
 */
export const DEFAULT_ENFORCEMENT_OPTIONS: CapabilityEnforcementOptions = {
  strict: true,
  warnOnly: false,
};
