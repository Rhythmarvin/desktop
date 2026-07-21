// This file runs as a side-effect on import — no manual call needed.
function format(args) {
    return args
        .map((a) => (typeof a === "object" ? JSON.stringify(a) : String(a)))
        .join(" ");
}
console.log = (...args) => {
    process.stderr.write(`[plugin] ${format(args)}\n`);
};
console.warn = (...args) => {
    process.stderr.write(`[plugin:warn] ${format(args)}\n`);
};
console.error = (...args) => {
    process.stderr.write(`[plugin:error] ${format(args)}\n`);
};
export {};
