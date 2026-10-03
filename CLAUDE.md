# bird

Self-hostable PaaS in Rust. Podman for containers, custom proxy, SQLite state, WireGuard multi-node later.
Infra model inspired by Fly.io (machines, edge proxy, private network), product model by Railway (projects, environments, services, variables).

## Workflow

* Requires Linux with Podman 5+ and its API socket enabled (`systemctl --user enable --now podman.socket` for rootless).
* Ship in small, working steps. Every step must build, pass clippy, and pass tests before moving on.

## Commands

* Build: `cargo build --release` produces `target/release/birdd` (daemon) and `target/release/bird` (CLI).
* Check: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
* Test: `cargo test -- --include-ignored` (ignored tests need a running Podman socket).
* Run: `birdd --data-dir <dir>`, then `bird login 127.0.0.1:7070 < <dir>/api-token` once, then `bird deploy <name> <image> --domain <host>`; the proxy listens on `:8080`, the API on `127.0.0.1:7070`.
* Service: `contrib/systemd/birdd.service` is a rootless user unit; install steps are in its header.

## Architecture

* Workspace crates under `crates/`. Dependency direction only flows downward:
  * `bird-core`: shared types, no I/O, no dependency on other bird crates.
  * `bird-store`, `bird-podman`, `bird-proxy`: depend only on `bird-core`.
  * `bird-api`: HTTP request/response types shared by server and CLI; depends only on `bird-core`.
  * `bird-server`: the `birdd` daemon, wires everything together.
  * `bird-cli`: the `bird` binary; talks to `birdd` over its HTTP API using `bird-api` types, never to the store or Podman directly.
* No circular dependencies, no "utils" or "common" dumping-ground crates or modules.

## File structure

* One concern per file. `lib.rs`/`main.rs` only declares modules and re-exports the public API.
* Split by domain entity or responsibility (e.g. `services.rs`, `deployments.rs`, `transport.rs`), not by kind of code (no `types.rs`, `helpers.rs`, `misc.rs`).
* Aim for files under ~300 lines including tests. When a file grows past that, split it before adding more.
* Large `impl` blocks are split across files by concern (`impl Store` lives in each entity module).
* Private wire/serde structs live next to the code that uses them, not in shared modules.
* Unit tests sit at the bottom of the file they test; shared test fixtures go in a `#[cfg(test)] mod testing`.
* Integration tests live in `crates/<name>/tests/`, one file per scenario area.
* Data model: `project → environment → service → deployment → machine`, plus `domains` and `variables` on services.
* Desired state lives in SQLite; the server reconciles actual state (Podman) toward it. Reconciliation must be idempotent.

## Rust

* Edition 2024, stable toolchain, `rustfmt` defaults.
* Lints set in `[workspace.lints]`: `unsafe_code = "forbid"`, clippy `all` + `pedantic` as warnings, `-D warnings` in checks. Allow specific pedantic lints only with a one-line reason.
* No `unwrap`, `expect`, `panic!`, `todo!`, or indexing that can panic in non-test code. Use `expect` only for true invariants, with a message saying why it cannot fail.
* Errors: `thiserror` enums in library crates, `anyhow` only in binaries. Never discard errors silently; propagate with context or log them.
* Newtypes for IDs and validated values (`AppName`, `Domain`, `MachineId`). Parse and validate at boundaries, then trust the type.
* Enums for state (`MachineState::Starting | Running | Stopped | Failed`), not strings or booleans.
* Prefer borrowing over cloning, `&str` over `String` in parameters, iterators over manual loops when clearer.
* Keep functions small and single-purpose. Keep modules flat until they need structure.
* Public items need clear names; avoid comments unless the reason is non-obvious, then one line.

## Async and concurrency

* `tokio` runtime everywhere.
* Never block inside async code. SQLite runs on a dedicated thread or via `spawn_blocking`.
* Every network call, Podman call, and health check has a timeout.
* Never hold a lock across `.await`. Prefer message passing or `arc-swap` for read-heavy shared state.
* Channels are bounded. Spawned tasks are tracked and shut down gracefully on SIGTERM.

## Performance

* The proxy is the hot path: no per-request allocation beyond what is required, stream bodies and never buffer them fully, reuse upstream connections, route table read lock-free via `arc-swap`.
* Measure before optimizing. No premature caching or abstraction.
* Release profile: `lto = "thin"`, `codegen-units = 1`.

## Safety and security

* Validate all external input: names (`^[a-z][a-z0-9-]{0,62}$`), domains, image refs, ports, env var keys.
* SQL uses bound parameters only, never string formatting.
* Never shell out with interpolated strings. If a process must be spawned, use `Command` with separate args.
* Secrets and env var values are never logged or returned in list responses.
* Limits on request body size, header size, and connection count in both API and proxy.
* TLS via `rustls`, never OpenSSL.
* The API binds to localhost or a unix socket by default; anything public requires auth.

## State

* SQLite in WAL mode, `foreign_keys = ON`, `busy_timeout` set.
* Versioned migrations embedded in the binary, applied on startup, never edited after release.
* Multi-row changes happen in a transaction.

## Dependencies

* Declared once in `[workspace.dependencies]`, crates use `workspace = true`.
* Prefer few, well-maintained crates. `default-features = false` and enable only what is used.
* Justify any new dependency; do not add one for something a few lines of code can do.

## Observability

* `tracing` with structured fields (`machine_id`, `service`, `deployment_id`), no `println!` outside the CLI.
* Log state transitions at `info`, retries at `warn`, failures at `error`.

## Testing

* Unit tests for pure logic (validation, routing, reconciliation decisions) next to the code.
* Integration tests run against real Podman and are marked `#[ignore = "requires a running podman socket"]`; no mocking Podman except behind a trait for unit tests.
* Verify real API shapes (e.g. `curl --unix-socket`) before writing wire types; never guess.
* A step is done when `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test -- --include-ignored` pass.

## Git

* One-line conventional commits, lowercase, imperative, no body, no attribution.
* Commit only when the user asks.
