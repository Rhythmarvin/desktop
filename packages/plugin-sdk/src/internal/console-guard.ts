/**
 * Console guard — redirects console.* to stderr.
 * Runs at import time (side-effect only). This is NOT a sandbox;
 * a malicious same-user plugin can bypass by directly accessing fd 1.
 * Tampering triggers a fatal protocol violation on the Host side.
 */

const originalStderrWrite = process.stderr.write.bind(process.stderr);

function format(args: unknown[]): string {
  return args
    .map((a) => {
      if (typeof a === "string") return a;
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(" ");
}

console.log = (...args: unknown[]): void => {
  originalStderrWrite(`[plugin] ${format(args)}\n`);
};

console.warn = (...args: unknown[]): void => {
  originalStderrWrite(`[plugin:warn] ${format(args)}\n`);
};

console.error = (...args: unknown[]): void => {
  originalStderrWrite(`[plugin:error] ${format(args)}\n`);
};
