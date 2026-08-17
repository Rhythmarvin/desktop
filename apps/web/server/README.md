# ora-web-server

`ora-web-server` is the Web composition root for Ora's shared backend, HTTP contracts, filesystem adapters, plugin discovery, process logging, and readiness lifecycle.

## Runtime boundaries

- Environment parsing supplies a provisional `ORA_LOG_LEVEL` or `info` filter before Backend bootstrap so migration diagnostics remain observable.
- After SQLite migration, `user_config.log_level` supplies the preferred level when no environment override exists; `user_config.developer_mode` owns shared developer-setting discoverability.
- `AppState` shares the backend and process-wide runtime settings manager across all HTTP requests.
- Mutating the runtime log level affects the entire Web server process and every connected client, not an individual browser session.

## Failure semantics

A malformed persisted `log_level` is fatal because startup reads it before readiness. A malformed `developer_mode` is reported when its route reads that preference; startup does not eagerly load it. Live log-level updates finish in a cancellation-safe internal transaction, reload first, and atomically persist second; persistence failure attempts to restore the previous live filter while preserving storage as the primary client-visible failure. Developer mode is a UI discoverability preference, not an authorization boundary.
