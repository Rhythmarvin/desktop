// console-guard.ts — Redirect console.* to stderr to protect stdout frame channel.
// This file executes at import time. Import it before any other plugin-sdk module.

const stderr = process.stderr.write.bind(process.stderr);

function fmt(args: unknown[]): string {
  return args.map((a) => (typeof a === "object" ? JSON.stringify(a) : String(a))).join(" ");
}

console.log = (...args: unknown[]) => stderr(`[plugin] ${fmt(args)}\n`);
console.warn = (...args: unknown[]) => stderr(`[plugin:warn] ${fmt(args)}\n`);
console.error = (...args: unknown[]) => stderr(`[plugin:error] ${fmt(args)}\n`);
