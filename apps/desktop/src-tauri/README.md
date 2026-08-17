# Ora Desktop

`ora-desktop` is the native Tauri host for Ora. It bootstraps the shared backend, exposes desktop-only commands to the frontend, owns native windows and dialogs, and adapts operating-system capabilities such as filesystem handoff and marketplace WebViews.

The crate does not own domain persistence or agent execution semantics; those remain in the shared backend and contract crates. Desktop commands translate between Tauri IPC and those stable boundaries.

Desktop installs provisional logging from `ORA_LOG_LEVEL` or `info` before Backend migration, then resolves the runtime-ready level from the shared SQLite `user_config.log_level` preference when no environment override exists. It retains the logging writer guard for the process lifetime and shares a cancellation-safe runtime settings manager through Tauri state.

The shared developer preferences are exposed through `get_developer_mode`, `set_developer_mode`, `get_runtime_log_level`, and `set_runtime_log_level`. These commands enter a request-correlated span before reading, persisting, reloading, or compensating; SQLite ownership remains in Backend rather than this crate. Desktop `config.json` does not store either preference.

Native marketplace windows use isolated browser profiles and provider-specific navigation policies. Their download events are routed into Ora-owned application data before the frontend is notified.

Ripgrep and Deno are bundled as Tauri sidecars under `binaries/rg` and
`binaries/deno`. Their platform-specific executables are downloaded by
`scripts/setup-binary.mjs` during dependency installation and are intentionally
excluded from version control. `BundledBinaryPaths` resolves both paths during
Desktop bootstrap and stores them in `DesktopState`; the shared backend and
`ora-fs` receive the resolved ripgrep path, while Rust-owned Deno integrations
receive the resolved Deno path. If either executable is missing, Desktop logs
the failure and stops before constructing the application state.

On Windows, `build.rs` omits Tauri's resource-embedded app manifest and instead
attaches the Common-Controls v6 side-by-side dependency via the linker for every
artifact (including `cargo test` harnesses). Without that, the lib-test binary binds
legacy comctl32 and fails to load with `STATUS_ENTRYPOINT_NOT_FOUND`.
