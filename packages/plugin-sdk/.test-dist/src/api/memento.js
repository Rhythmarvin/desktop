/**
 * Create a Memento instance backed by an in-memory cache.
 *
 * @param initialData  Initial key-value pairs from `$/init.globalState`.
 * @param rpc          RPC client used to notify the Host of `update()` calls.
 *
 * Read path: `get()` returns directly from the cache (synchronous, zero I/O).
 * Write path: `update()` writes to the cache first, then fires an async
 * `ora.storage.set` notification to the Host for persistence.
 */
export function createMemento(initialData, rpc) {
    const cache = { ...initialData };
    return {
        get(key, defaultValue) {
            const value = cache[key];
            if (value === undefined)
                return defaultValue;
            return value;
        },
        async update(key, value) {
            if (value === undefined) {
                throw new Error("Cannot store undefined value. Use null for intentional absence.");
            }
            // Step 1: update memory immediately (subsequent get() calls see new value)
            cache[key] = value;
            // Step 2: notify Host asynchronously (fire-and-forget — don't block on disk I/O)
            rpc.notify("ora.storage.set", { key, value });
        },
        keys() {
            return Object.keys(cache);
        },
    };
}
