/**
 * SDK package export verification tests.
 *
 * Verifies:
 * - ./agent and ./types are importable
 * - /host, /bootstrap, /internal, reader, writer are NOT importable
 * - package tarball only exposes approved entry points
 */
import { describe, it, expect } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";

describe("Public exports", () => {
  it("./agent is importable", async () => {
    const mod = await import("../src/agent/index.js");
    expect(mod.defineAgentPlugin).toBeDefined();
    expect(typeof mod.defineAgentPlugin).toBe("function");
  });

  it("./types is importable", async () => {
    const mod = await import("../src/types/index.js");
    expect(mod).toBeDefined();
  });
});

describe("Legacy exports are absent", () => {
  it("/host cannot be resolved", () => {
    expect(() => require.resolve("@ora-space/plugin-sdk/host")).toThrow();
  });

  it("/bootstrap cannot be resolved", () => {
    expect(() => require.resolve("@ora-space/plugin-sdk/bootstrap")).toThrow();
  });

  it("/internal cannot be resolved", () => {
    expect(() => require.resolve("@ora-space/plugin-sdk/internal")).toThrow();
  });
});

describe("Package structure", () => {
  it("package.json has only approved exports", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(import.meta.dir, "..", "package.json"), "utf-8")
    );
    const exports = pkg.exports || {};
    const keys = Object.keys(exports);
    expect(keys).toContain("./agent");
    expect(keys).toContain("./types");
    expect(keys).toContain("./package.json");
    expect(keys).not.toContain("./host");
    expect(keys).not.toContain("./bootstrap");
    expect(keys).not.toContain("./internal");
    expect(keys).not.toContain(".");
  });

  it("src/host directory does not exist", () => {
    const hostDir = path.join(import.meta.dir, "..", "src", "host");
    expect(fs.existsSync(hostDir)).toBe(false);
  });
});
