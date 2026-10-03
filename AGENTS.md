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
* Run: `birdd --data-dir <dir>`, then `bird login 127.0.0.1:7070 < <dir>/api-token` once, then `bird deploy <name> <image> --domain <host>`; the proxy listens on `:80` (HTTPS `:443`), the API on `127.0.0.1:7070`. Rootless needs `net.ipv4.ip_unprivileged_port_start=80`, or pass `--proxy-addr`/`--https-addr` with high ports.
* Service: `contrib/systemd/birdd.service` is a rootless user unit; install steps are in its header.
* TLS: `birdd --acme-directory <url> [--acme-email <email>]` enables automatic certificates (HTTP-01 on the proxy port, HTTPS on `--https-addr`, default `:443`). Without it no HTTPS listener runs. `*.localhost` domains never get certificates.
* Testing TLS without a public IP: run Let's Encrypt's Pebble and pebble-challtestsrv in Podman, point challtestsrv at `host.containers.internal`, set Pebble's `httpPort` to the proxy port (80), and pass `--acme-ca-cert pebble.minica.pem`.
* API docs: `birdd` serves Scalar at `/docs` and the spec at `/v1/openapi.json` (both without auth). Handlers carry `#[utoipa::path]` and are registered with `routes!`; `docs/openapi.json` is committed and a test fails when it drifts, regenerate with `BIRD_UPDATE_OPENAPI=1 cargo test -p bird-server`. Scalar is pinned with an SRI hash in `crates/bird-server/src/api/docs.rs`; update the version and hash together.
* Benchmark the proxy with `crates/bird-proxy/examples/bench.rs` (usage in its header); compare proxied against direct before claiming a speedup.

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
* Private network: every machine joins the `bird` Podman network with aliases `<service>` and `<service>.internal`, so services call each other directly by name. DNS returns every replica's address in a fixed order, so spreading internal traffic across replicas is up to the client.
* Volumes: a service with volumes runs exactly one machine and deploys by stopping the old machine (state `Stopped`, kept for a restore if the new one fails) before starting the new one. Each volume records the image lineage (repository, major version, variant) that first used it and refuses another without `allow_image_change`. Volumes are only deleted by `bird rm --purge`.
* Variable references: values may contain `${{service.KEY}}`, `${{KEY}}` (same service) and `${{secret}}`/`${{secret(N)}}`. Secrets are generated once when set and stored as plain values; references are stored as written and resolved when a deploy starts, into the deployment snapshot's `resolved` column that machines run with. Changes are dry-run resolved, including services that depend on the changed one, before anything is saved.
* `bird.toml`: one service per file, parsed by the CLI into `bird_api::Manifest` and sent as a normal deploy; flags override it and repeatable flags add to its lists. Deploying from it only adds variables and domains, it never removes ones set elsewhere. Unknown fields are errors. Secrets stay out of it (`bird env set`).
* Builds: `[build]` in `bird.toml` or `bird deploy --build [dir]` packs the context as a gzipped tar (trimmed by `.dockerignore`, which podman applies again) and uploads it to `POST /v1/services/{name}/builds`; birdd builds it with podman and streams `BuildEvent`s back. Builds run one at a time (others queue) and take `[build] args` / `--build-arg` as Dockerfile ARGs, which stay readable in the image. Built images are tagged `localhost/bird/<service>:<millis>`, are never pulled, and the last few distinct images per service are kept for rollbacks.
* Machines run with podman's init as pid 1, so apps that ignore SIGTERM still stop promptly.
* Templates: built-in TOML files in `crates/bird-server/templates/` are `bird.toml` manifests plus `description` and `connection`; `{service}` is replaced with the new service name. They are embedded with `include_str!` and checked by tests; pin images to a major (or major.minor) tag.
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
