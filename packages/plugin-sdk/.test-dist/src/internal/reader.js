const decoder = new TextDecoder();
let lineBuffer = "";
const messageHandlers = [];
const errorHandlers = [];
const closeHandlers = [];
let listening = false;
/**
 * Register a message callback. Every time a complete JSON line is parsed from
 * stdin, all registered handlers are called with the parsed object.
 * Returns an unsubscribe function.
 */
export function onMessage(handler) {
    messageHandlers.push(handler);
    return () => {
        const idx = messageHandlers.indexOf(handler);
        if (idx >= 0)
            messageHandlers.splice(idx, 1);
    };
}
/**
 * Register an error callback. Called when a line cannot be parsed as JSON.
 * Returns an unsubscribe function.
 */
export function onError(handler) {
    errorHandlers.push(handler);
    return () => {
        const idx = errorHandlers.indexOf(handler);
        if (idx >= 0)
            errorHandlers.splice(idx, 1);
    };
}
/**
 * Register a close callback. Called when stdin closes (EOF, Host process exits).
 * Returns an unsubscribe function.
 */
export function onClose(handler) {
    closeHandlers.push(handler);
    return () => {
        const idx = closeHandlers.indexOf(handler);
        if (idx >= 0)
            closeHandlers.splice(idx, 1);
    };
}
/**
 * Start listening on stdin. Idempotent — subsequent calls are no-ops.
 * Runs a continuous `for await` loop that parses newline-delimited JSON
 * and delivers each parsed message to all registered handlers.
 *
 * This function does not resolve until stdin closes, so callers should
 * NOT await it if they need to do other work concurrently.
 */
export async function startListening() {
    return _startListeningWithSource(process.stdin);
}
/**
 * Internal: start listening on a specific byte source. Exposed so tests can
 * inject a mock async iterable in place of process.stdin.
 *
 * Not part of the public SDK API.
 */
export async function _startListeningWithSource(source) {
    if (listening)
        return;
    listening = true;
    try {
        for await (const chunk of source) {
            lineBuffer += decoder.decode(chunk, { stream: true });
            while (true) {
                const idx = lineBuffer.indexOf("\n");
                if (idx < 0)
                    break;
                const line = lineBuffer.slice(0, idx);
                lineBuffer = lineBuffer.slice(idx + 1);
                if (line.length === 0) {
                    const err = new SyntaxError("Unexpected empty line on stdin");
                    for (const handler of errorHandlers) {
                        try {
                            handler(err);
                        }
                        catch { /* swallow */ }
                    }
                    continue;
                }
                try {
                    const message = JSON.parse(line);
                    for (const handler of messageHandlers) {
                        try {
                            handler(message);
                        }
                        catch { /* swallow — one handler's error must not block others */ }
                    }
                }
                catch (parseError) {
                    const err = parseError instanceof Error ? parseError : new Error(String(parseError));
                    for (const handler of errorHandlers) {
                        try {
                            handler(err);
                        }
                        catch { /* swallow */ }
                    }
                }
            }
        }
        // EOF — yield remaining buffer content if any
        if (lineBuffer.length > 0) {
            try {
                const message = JSON.parse(lineBuffer);
                for (const handler of messageHandlers) {
                    try {
                        handler(message);
                    }
                    catch { /* swallow */ }
                }
            }
            catch (parseError) {
                const err = parseError instanceof Error ? parseError : new Error(String(parseError));
                for (const handler of errorHandlers) {
                    try {
                        handler(err);
                    }
                    catch { /* swallow */ }
                }
            }
            lineBuffer = "";
        }
    }
    finally {
        // Always fire close handlers, even if the loop breaks on error
        for (const handler of closeHandlers) {
            try {
                handler();
            }
            catch { /* swallow */ }
        }
    }
}
/**
 * Reset all module-level state. Exposed for testing only — not part of
 * the public SDK API.
 */
export function __resetForTesting() {
    messageHandlers.length = 0;
    errorHandlers.length = 0;
    closeHandlers.length = 0;
    lineBuffer = "";
    listening = false;
}
