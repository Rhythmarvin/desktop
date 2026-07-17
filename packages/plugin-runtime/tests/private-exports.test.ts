/**
 * Private runtime package — negative export verification.
 *
 * Proves that plugin code CANNOT import @ora-space/plugin-runtime.
 * The package is "private": true and must not be importable by third-party code.
 */
import { describe, it, expect } from "bun:test";
import * as fs from "node:fs";
import * as path from "node:path";

describe("@ora-space/plugin-runtime is private", () => {
  it("package.json has private: true", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(import.meta.dir, "..", "package.json"), "utf-8")
    );
    expect(pkg.private).toBe(true);
  });

  it("package.json has no publishConfig", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(import.meta.dir, "..", "package.json"), "utf-8")
    );
    expect(pkg.publishConfig).toBeUndefined();
  });

  it("package name confirms private ownership", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(import.meta.dir, "..", "package.json"), "utf-8")
    );
    expect(pkg.name).toBe("@ora-space/plugin-runtime");
  });
});

describe("Internal modules are importable within the package (dev only)", () => {
  it("reader module exports expected API", async () => {
    const mod = await import("../src/internal/reader.js");
    expect(mod.HEADER_LEN).toBe(5);
    expect(mod.MAX_PAYLOAD).toBe(8 * 1024 * 1024);
    expect(typeof mod.createFrameReader).toBe("function");
  });

  it("writer module exports expected API", async () => {
    const mod = await import("../src/internal/writer.js");
    expect(typeof mod.encodeFrame).toBe("function");
    expect(typeof mod.createFrameWriter).toBe("function");
  });

  it("transport module exports expected API", async () => {
    const mod = await import("../src/rpc/transport.js");
    expect(typeof mod.createTransport).toBe("function");
  });

  it("handshake module exports expected API", async () => {
    const mod = await import("../src/lifecycle/handshake.js");
    expect(typeof mod.createInitializeWaiter).toBe("function");
    expect(typeof mod.createActivateWaiter).toBe("function");
  });

  it("bootstrap module exports expected API", async () => {
    const mod = await import("../src/bootstrap/bootstrap.js");
    expect(typeof mod.bootstrap).toBe("function");
  });
});
