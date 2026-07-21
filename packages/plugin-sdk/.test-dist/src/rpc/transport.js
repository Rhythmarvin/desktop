import { onMessage as readerOnMessage, onError as readerOnError, onClose as readerOnClose, startListening, } from "../internal/reader.js";
import { writeLine } from "../internal/writer.js";
/**
 * Wraps the stdin reader and stdout writer into a unified message transport.
 *
 * - `start()` initializes stdin listening (idempotent).
 * - `send(message)` writes a JSON object to stdout followed by `\n`.
 * - `onMessage / onClose / onError` delegate to the reader's handler
 *   registration, each returning an unsubscribe function.
 */
export class Transport {
    started = false;
    /** Start the background stdin listener. Idempotent. */
    start() {
        if (this.started)
            return;
        this.started = true;
        // Fire-and-forget: the promise resolves only when stdin closes.
        // Callers should NOT await it — they receive messages via onMessage.
        void startListening();
    }
    /** Write a JSON object as a newline-terminated line to stdout. */
    send(message) {
        writeLine(JSON.stringify(message));
    }
    /** Register a message handler. Returns an unsubscribe function. */
    onMessage(handler) {
        return readerOnMessage(handler);
    }
    /** Register a close handler. Returns an unsubscribe function. */
    onClose(handler) {
        return readerOnClose(handler);
    }
    /** Register an error handler. Returns an unsubscribe function. */
    onError(handler) {
        return readerOnError(handler);
    }
}
