# Memory Sentry Rust companion

The Rust companion provides the Windows process-management layer and local API used by the Spicetify Custom App. Its responsibilities include process identity checks, working-set operations, adaptive policy execution, recovery persistence, and per-user lifecycle management.

The release is experimental. Unit tests and successful compilation do not replace real-desktop validation of Spotify playback, foreground restoration, recovery, and uninstall.

## Build and test

Install a stable Rust toolchain. Run these commands from the repository root:

```powershell
cargo test --manifest-path native/rust/Cargo.toml --locked
cargo build --release --manifest-path native/rust/Cargo.toml --target x86_64-pc-windows-msvc --locked
```

The `--version` and `--self-test` commands do not change process memory. The launcher checks the authenticated local status endpoint and records early startup diagnostics under `%LOCALAPPDATA%\SpicetifyMemorySentry\logs\startup.log`.

Uninstall attempts recovery before removing the Custom App. If restoration cannot be confirmed, it preserves recovery data and stops removal.

See [Server migration and release gate](../../docs/RUST-MIGRATION.md) and the [Release checklist](../../docs/RELEASE_CHECKLIST.md).
