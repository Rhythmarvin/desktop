/**
 * Pack CLI — produces a materialized single-file ESM bundle from plugin source.
 *
 * Equivalent to: bun build src/index.ts --target=bun --format=esm --packages=bundle --outfile dist/index.js
 *
 * The output goes to a staging directory outside the source tree.
 * Private runtime/internal modules are excluded from the bundle.
 */

import * as path from "node:path";
import * as fs from "node:fs";

export interface PackOptions {
  /** Source directory containing package.json. */
  sourceDir: string;
  /** Output staging directory (must be outside sourceDir). */
  outputDir: string;
  /** Path to the Bun executable. */
  bunPath?: string;
  /** Entry file relative to sourceDir (default: src/index.ts). */
  entry?: string;
}

export interface PackResult {
  success: boolean;
  outputFile?: string;
  metafilePath?: string;
  errors: string[];
}

/** Run the pack command. */
export async function pack(options: PackOptions): Promise<PackResult> {
  const errors: string[] = [];
  const { sourceDir, outputDir, entry = "src/index.ts" } = options;
  const bunExe = options.bunPath ?? "bun";

  // ── Validate inputs ─────────────────────────────────────────
  const absSource = path.resolve(sourceDir);
  const absOutput = path.resolve(outputDir);

  // Output must not be inside source
  if (absOutput.startsWith(absSource + path.sep) || absOutput === absSource) {
    errors.push("output directory must be outside source tree");
    return { success: false, errors };
  }

  // Source must have a package.json
  const pkgPath = path.join(absSource, "package.json");
  if (!fs.existsSync(pkgPath)) {
    errors.push(`package.json not found at ${pkgPath}`);
    return { success: false, errors };
  }

  // Source must have the entry file
  const entryPath = path.join(absSource, entry);
  if (!fs.existsSync(entryPath)) {
    errors.push(`entry file not found: ${entryPath}`);
    return { success: false, errors };
  }

  // ── Create staging directory ────────────────────────────────
  fs.mkdirSync(absOutput, { recursive: true });

  const outfile = path.join(absOutput, "dist", "index.js");
  fs.mkdirSync(path.dirname(outfile), { recursive: true });

  const metafilePath = path.join(absOutput, "meta.json");

  // ── Run Bun build ───────────────────────────────────────────
  try {
    const proc = Bun.spawnSync({
      cmd: [
        bunExe,
        "build",
        entry,
        "--target=bun",
        "--format=esm",
        "--packages=bundle",
        `--outfile=${outfile}`,
        `--metafile=${metafilePath}`,
      ],
      cwd: absSource,
      stdout: "pipe",
      stderr: "pipe",
    });

    if (proc.exitCode !== 0) {
      const stderr = new TextDecoder().decode(proc.stderr);
      errors.push(`bun build failed (exit ${proc.exitCode}): ${stderr}`);
      return { success: false, errors };
    }
  } catch (err) {
    errors.push(`failed to spawn bun: ${(err as Error).message}`);
    return { success: false, errors };
  }

  // ── Verify output ───────────────────────────────────────────
  if (!fs.existsSync(outfile)) {
    errors.push(`build succeeded but output file not found: ${outfile}`);
    return { success: false, errors };
  }

  // Copy package.json to staging
  fs.copyFileSync(pkgPath, path.join(absOutput, "package.json"));

  return {
    success: true,
    outputFile: outfile,
    metafilePath: fs.existsSync(metafilePath) ? metafilePath : undefined,
    errors: [],
  };
}

// ── CLI entry point ─────────────────────────────────────────────

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.length < 2) {
    console.error("Usage: bun run pack <sourceDir> <outputDir> [--entry src/index.ts]");
    process.exit(1);
  }

  const sourceDir = args[0];
  const outputDir = args[1];
  const entryIdx = args.indexOf("--entry");
  const entry = entryIdx >= 0 ? args[entryIdx + 1] : undefined;

  const result = await pack({ sourceDir, outputDir, entry });
  if (result.success) {
    console.log(`Packed: ${result.outputFile}`);
    if (result.metafilePath) console.log(`Metafile: ${result.metafilePath}`);
  } else {
    console.error("Pack failed:");
    for (const err of result.errors) console.error(`  - ${err}`);
    process.exit(1);
  }
}
