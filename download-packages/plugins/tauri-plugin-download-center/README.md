# tauri-plugin-download-center

Register `init(database)` with an adapter for the host's existing SQLite pool.
The plugin directly owns `list`, `start`, `pause`, `resume`, `retry`, `cancel`
and `remove`; hosts no longer implement a command-forwarding service. Plugin
setup eagerly applies the versioned shared schema, and every command also waits
for that one-time initialization before it executes;
it never creates a second database file. Existing hosts should retain already
applied sqlx migration files as migration history.

The crate owns the persistent task model, repository, instance-level concurrency
and control state, Range resume, cancellation, checksum verification, and safe
ZIP installation. Hosts provide an ordered execution plan containing only the
download, verification, extraction, backup, replacement, and cleanup operations
needed by a task. Rollback runs only when a backup instruction was used.

`init_with_config` configures maximum concurrency and progress event throttling;
defaults are three tasks and 200 ms.

Add `download-center:default` to the Tauri capability.

## Awaitable overlay lifecycle

Hosts that manage running services can register
`init_with_install_hook(database, config, Some(Arc::new(hook)))`.
Implement `manager::InstallHook::before_overlay(app, task, target)` to validate
ownership and await a safe stop. It runs after download, verification and
extraction, immediately before `overlayTarget` copies files. An error prevents
the copy; it must not be replaced by a frontend status watcher.

Return an optional boxed `InstallGuard` to hold a maintenance lock across the
copy. The manager awaits `guard.finish(success)` before finishing installation.
Cancellation observed before copying calls `finish(false)` without touching the
target. Hosts should also implement a synchronous, fail-closed `Drop` fallback
for interruption. Once copying starts, do not restart services until copying
has actually finished. Hooks are not invoked for `replaceTarget`.

The hook is registered Rust code, not an executable callback in downloaded
metadata. Existing `init` and `init_with_config` callers remain compatible.
Overlay preserves unrelated target files; unlike a backed-up replacement it
does not promise transactional rollback of a partially copied package.
