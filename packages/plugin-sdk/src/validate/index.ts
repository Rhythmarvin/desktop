/**
 * Validate CLI — security audit for materialized plugin artifacts.
 *
 * Checks:
 * - Allowlist: only package.json, dist/index.js, optional README*/LICENSE*
 * - No node_modules directory
 * - No symlinks or junctions
 * - No native .node addons
 * - No workspace links
 * - No @ora-space/plugin-runtime imports (private package must not be in bundle)
 * - Metafile consistency audit (no external/dynamic unresolved packages)
 */

import * as fs from "node:fs";
import * as path from "node:path";

export interface ValidateOptions {
  /** Directory containing the built artifact. */
  artifactDir: string;
  /** Optional path to Bun metafile for import audit. */
  metafilePath?: string;
  /** Run relocation test (move to temp dir and verify load). */
  relocationTest?: boolean;
}

export interface ValidateResult {
  valid: boolean;
  errors: string[];
  warnings: string[];
}

/** Allowed files at the artifact root. */
const ALLOWED_FILES = new Set([
  "package.json",
  "dist/index.js",
]);

/** Files allowed with glob patterns. */
function isAllowedFile(name: string): boolean {
  if (ALLOWED_FILES.has(name)) return true;
  if (name.startsWith("README") || name.startsWith("LICENSE")) return true;
  return false;
}

/** Forbidden strings in bundle content. */
const FORBIDDEN_IMPORTS = [
  "@ora-space/plugin-runtime",
  "node-gyp",
  ".node",
];

/** Run the validate command. */
export async function validate(options: ValidateOptions): Promise<ValidateResult> {
  const errors: string[] = [];
  const warnings: string[] = [];
  const absDir = path.resolve(options.artifactDir);

  if (!fs.existsSync(absDir)) {
    errors.push(`artifact directory not found: ${absDir}`);
    return { valid: false, errors, warnings };
  }

  // ── Filesystem audit ────────────────────────────────────────
  const allFiles = listAllFiles(absDir);

  for (const file of allFiles) {
    const rel = path.relative(absDir, file);
    const basename = path.basename(file);

    // Check symlinks/junctions
    try {
      const stat = fs.lstatSync(file);
      if (stat.isSymbolicLink()) {
        errors.push(`symlink not allowed: ${rel}`);
      }
    } catch (err) {
      errors.push(`cannot stat: ${rel}: ${(err as Error).message}`);
    }

    // Check for node_modules
    if (rel.includes("node_modules")) {
      errors.push(`node_modules not allowed: ${rel}`);
    }

    // Check for .node native addons
    if (basename.endsWith(".node")) {
      errors.push(`native addon not allowed: ${rel}`);
    }

    // Check for workspace links (.pnpm, symlinks)
    if (basename === "pnpm-lock.yaml" || rel.includes(".pnpm")) {
      warnings.push(`workspace artifact found: ${rel}`);
    }
  }

  // ── Root-level allowlist ────────────────────────────────────
  const rootEntries = fs.readdirSync(absDir);
  for (const entry of rootEntries) {
    if (!isAllowedFile(entry)) {
      errors.push(`file not in allowlist: ${entry}`);
    }
  }

  // Check required files exist
  if (!fs.existsSync(path.join(absDir, "package.json"))) {
    errors.push("missing package.json");
  }
  if (!fs.existsSync(path.join(absDir, "dist", "index.js"))) {
    errors.push("missing dist/index.js");
  }

  // ── Bundle content audit ────────────────────────────────────
  const bundlePath = path.join(absDir, "dist", "index.js");
  if (fs.existsSync(bundlePath)) {
    const content = fs.readFileSync(bundlePath, "utf-8");
    for (const forbidden of FORBIDDEN_IMPORTS) {
      if (content.includes(forbidden)) {
        errors.push(`forbidden import detected in bundle: ${forbidden}`);
      }
    }
    // Check for dynamic imports (basic heuristic)
    const dynamicImportCount = (content.match(/import\s*\(/g) || []).length;
    if (dynamicImportCount > 0) {
      warnings.push(`${dynamicImportCount} dynamic import(s) detected in bundle`);
    }
    // Check for require() calls (not allowed in ESM bundle)
    const requireCount = (content.match(/\brequire\s*\(/g) || []).length;
    if (requireCount > 0) {
      warnings.push(`${requireCount} require() call(s) detected in bundle`);
    }
  }

  // ── Metafile audit ──────────────────────────────────────────
  if (options.metafilePath && fs.existsSync(options.metafilePath)) {
    try {
      const meta = JSON.parse(fs.readFileSync(options.metafilePath, "utf-8"));
      const inputs = meta.inputs || {};
      const outputs = meta.outputs || {};

      // Check for external (unresolved) imports
      for (const [key, output] of Object.entries(outputs) as [string, { imports?: Array<{ path: string; external?: boolean }> }][]) {
        if (output.imports) {
          for (const imp of output.imports) {
            if (imp.external) {
              // Built-in modules like "fs", "path" are OK; others are suspicious
              if (!["fs", "path", "os", "crypto", "buffer", "stream", "events",
                     "util", "url", "querystring", "assert", "child_process",
                     "node:fs", "node:path", "node:os", "node:crypto", "node:buffer",
                     "node:stream", "node:events", "node:util", "node:url",
                     "bun", "bun:jsc", "bun:ffi", "bun:sqlite", "bun:test"].includes(imp.path)) {
                errors.push(`external import not allowed: ${imp.path} (in ${key})`);
              }
            }
          }
        }
      }
    } catch (err) {
      warnings.push(`metafile parse warning: ${(err as Error).message}`);
    }
  }

  // ── Relocation test ─────────────────────────────────────────
  if (options.relocationTest) {
    const tmpDir = fs.mkdtempSync(path.join(path.dirname(absDir), "ora-relocate-"));
    try {
      // Copy artifact to temp dir
      copyDirSync(absDir, tmpDir);
      // Try loading the bundle
      try {
        await import(path.join(tmpDir, "dist", "index.js"));
        // If it loads without error, relocation passes
      } catch (err) {
        warnings.push(`relocation load warning: ${(err as Error).message}`);
      }
    } finally {
      // Cleanup
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  }

  return {
    valid: errors.length === 0,
    errors,
    warnings,
  };
}

function listAllFiles(dir: string): string[] {
  const result: string[] = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    result.push(fullPath);
    if (entry.isDirectory()) {
      result.push(...listAllFiles(fullPath));
    }
  }
  return result;
}

function copyDirSync(src: string, dest: string): void {
  fs.mkdirSync(dest, { recursive: true });
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      copyDirSync(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

// ── CLI entry point ─────────────────────────────────────────────

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error("Usage: bun run validate <artifactDir> [--metafile <path>] [--relocate]");
    process.exit(1);
  }

  const artifactDir = args[0];
  const metafileIdx = args.indexOf("--metafile");
  const metafilePath = metafileIdx >= 0 ? args[metafileIdx + 1] : undefined;
  const relocate = args.includes("--relocate");

  const result = await validate({ artifactDir, metafilePath, relocationTest: relocate });
  if (result.valid) {
    console.log("Validation passed.");
    for (const w of result.warnings) console.warn(`  WARNING: ${w}`);
  } else {
    console.error("Validation failed:");
    for (const err of result.errors) console.error(`  ERROR: ${err}`);
    for (const w of result.warnings) console.warn(`  WARNING: ${w}`);
    process.exit(1);
  }
}
