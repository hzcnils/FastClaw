# tauri-download-center-vue

Single-package frontend entry for the download center. It re-exports the core
client and registers all Vue components. Text and locale messages can be
overridden at installation time.

The Vue plugin creates an isolated download runtime and talks only to
`plugin:download-center|execute`. It does not import the account package
or a host download store.

Both IPC and event transports are injectable. The default Tauri modules are
loaded lazily, so browser previews and tests can install the package with mocks.
Create the runtime explicitly with `createDownloadRuntime()`, retain that instance
in the host adapter, and pass it to the plugin as `runtimeInstance`. Components
receive the same instance through Vue injection. Completion and failure side
effects belong in `onTaskCompleted`/`onTaskFailed` hooks rather than the UI.

The host places an `executionPlan` in `StartDownloadInput.payload`. Its ordered
instructions can download, verify SHA-256, extract ZIP files, back up or replace
targets, and clean temporary paths. Resource URLs and filesystem paths remain
host-owned configuration.
