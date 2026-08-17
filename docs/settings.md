# Settings

Ora's App Shell presents the same Settings experience in Web and Desktop through the shared contracts client.

## Developer mode

Settings always includes an Advanced category, and Advanced contains the developer-mode switch. Its authoritative value is the typed SQLite `user_config.developer_mode` preference; the frontend does not persist a second copy. A failed initial read leaves the switch disabled, keeps Developer options hidden, and offers retry. A failed update retains the last backend response.

Developer mode controls discoverability only. It does not grant permissions, change transport authorization, or make backend operations inaccessible when disabled.

## Developer options

The Developer options navigation category exists only while developer mode is enabled. If the authoritative value becomes disabled while the category is selected, Settings immediately falls back to Advanced and unmounts the developer-only content.

Developer options contains the process-wide log-level selector. Changes take effect for all clients of the current Web server process or for the current Desktop process and are persisted in `user_config.log_level`. The selector displays the authoritative effective level, including an active startup override, without naming `ORA_LOG_LEVEL` or exposing startup-source details. Trace and Debug include a high-volume warning.

See [Runtime Logging](runtime-logging.md) for startup precedence and rollback behavior.
