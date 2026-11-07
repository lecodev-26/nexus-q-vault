# Automatic vault locking

The desktop application enforces an inactivity timeout in the Rust process. The
default is five minutes; users can select one, five, ten, fifteen or thirty
minutes in Settings. The selected value is stored in the local application
preferences and validated again by the Rust command before it is applied.
No activity data is transmitted or persisted.

Keyboard, pointer, touch and wheel activity is reported to Rust with a
throttled IPC call. Rust owns the deadline and checks it independently of the
webview. The watchdog takes the same session mutex used by cryptographic
operations and rechecks the deadline after acquiring it. An operation already
holding the session therefore completes before the session can be locked, and
a new activity update can cancel a pending timeout.

The vault is also locked when the application exits. When Tauri reports an
event-loop resume, the app attempts to lock any still-open session before
refreshing the inactivity baseline. The frontend clears password fields and
updates the locked status when it receives the lock event. Lock errors are
surfaced to the interface; an exit request is prevented if saving the session
fails.

## Platform limitation

Tauri 2 documents its native suspended/resumed lifecycle events as mobile-only;
they are not available on Linux, macOS or Windows. The desktop app therefore
does not claim reliable detection of an operating-system screen lock or sleep
on those platforms. Hiding the webview (for example, minimizing the window)
also requests an immediate lock as a best-effort additional safeguard.
