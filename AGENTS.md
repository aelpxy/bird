# bird

Self-hostable PaaS in Rust. Podman for containers, custom proxy, SQLite state, multi-node later.
Infra model inspired by Fly.io (machines, edge proxy, private network), product model by Railway (projects, environments, services, variables).

## Workflow

* Requires Linux with Podman 5+ and its API socket enabled (`systemctl --user enable --now podman.socket` for rootless).
* Ship in small, working steps. Every step must build, pass clippy, and pass tests before moving on.

## Commands

* Build: `cargo build --release` produces `target/release/birdd` (daemon) and `target/release/bird` (CLI).
* Check: `cargo fmt --check && cargo clippy --all-targets -- -D warnings`
* Test: `cargo test -- --include-ignored` (ignored tests need a running Podman socket). `crates/bird-server/tests/` drive an in-process birdd (`tests/support`) over HTTP; its project names carry a prefix so networks never clash with another birdd on the same podman. CI runs every test in a privileged Fedora container, since Ubuntu's podman is 4.x.
* Run: `birdd --data-dir <dir>`, then `bird login 127.0.0.1:7070 < <dir>/api-token` once, then `bird deploy <name> <image> --domain <host>`; the proxy listens on `:80` (HTTPS `:443`), the API on `127.0.0.1:7070`. Rootless needs `net.ipv4.ip_unprivileged_port_start=80`, or pass `--proxy-addr`/`--https-addr` with high ports.
* Install: `contrib/setup.sh` builds, installs and starts the rootless user unit in `contrib/systemd/birdd.service`.
* TLS: `birdd --acme-directory <url> [--acme-email <email>]` enables automatic certificates (HTTP-01 on the proxy port, HTTPS on `--https-addr`, default `:443`). Without it no HTTPS listener runs. `*.localhost` domains never get certificates.
* Testing TLS without a public IP: run Let's Encrypt's Pebble and pebble-challtestsrv in Podman, point challtestsrv at `host.containers.internal`, set Pebble's `httpPort` to the proxy port (80), and pass `--acme-ca-cert pebble.minica.pem`.
* API docs: Scalar at `/docs`, spec at `/v1/openapi.json`. Handlers carry `#[utoipa::path]`; `docs/openapi.json` is committed and a test fails when it drifts, regenerate with `BIRD_UPDATE_OPENAPI=1 cargo test -p bird-server`. Scalar is pinned with an SRI hash in `api/docs.rs`; update version and hash together.
* Benchmark the proxy with `crates/bird-proxy/examples/bench.rs` (usage in its header); compare proxied against direct before claiming a speedup.

## Architecture

* Workspace crates under `crates/`. Dependency direction only flows downward:
  * `bird-core`: shared types, no I/O, no dependency on other bird crates.
  * `bird-store`, `bird-podman`, `bird-proxy`: depend only on `bird-core`.
  * `bird-api`: HTTP request/response types shared by server and CLI; depends only on `bird-core`.
  * `bird-server`: the `birdd` daemon, wires everything together.
  * `bird-cli`: the `bird` binary; talks to `birdd` over its HTTP API using `bird-api` types, never to the store or Podman directly. Service commands take the service from `-s` or `./bird.toml`, never a positional name (only `deploy`, `init` and `add` name new services). Results go to stdout and progress, spinners and prompts to stderr; read commands honour `--json`; color only on a terminal without `NO_COLOR`; destructive commands confirm unless `-y`, and refuse without a terminal.
* No circular dependencies, no "utils" or "common" dumping-ground crates or modules.

## File structure

* One concern per file. `lib.rs`/`main.rs` only declares modules and re-exports the public API.
* Split by domain entity or responsibility (e.g. `services.rs`, `deployments.rs`, `transport.rs`), not by kind of code (no `types.rs`, `helpers.rs`, `misc.rs`).
* Aim for files under ~300 lines including tests. When a file grows past that, split it before adding more.
* Large `impl` blocks are split across files by concern (`impl Store` lives in each entity module).
* Private wire/serde structs live next to the code that uses them, not in shared modules.
* Unit tests sit at the bottom of the file they test; shared test fixtures go in a `#[cfg(test)] mod testing`.
* Integration tests live in `crates/<name>/tests/`, one file per scenario area.

## Domain

* Model: `org → project → environment → service → deployment → machine`; domains, variables and volumes belong to services. Desired state lives in SQLite and birdd reconciles Podman toward it; reconciliation must be idempotent.
* Auth: a bearer token is an API token (`bird_`), a session (`birds_`) or the root token in the data dir, which always works and cannot be deleted. Token and session secrets are stored only hashed. Password hashes and 2fa secrets never sit on `User`; `LoginSecrets` carries them and redacts itself. `POST /v1/login` is the only route outside the auth layer.
* Access: server admins act as owners of every org; org roles are `owner > admin > member`. `api::scope::Scope` checks the project's org before resolving the environment, and projects and orgs a user does not belong to answer 404, so their names do not leak. An org's last owner can neither leave nor be deleted while the org owns projects.
* Environments: each has its own Podman network, created with `isolate=strict` because bridges otherwise route into each other by address. References, aliases and backups stay inside an environment; domains, registries and built images are global. Project names are unique server-wide, so paths and networks need no org.
* Deploys move traffic only once new machines pass their health check (sent with `Host: localhost`), and a failed deploy restores the previous settings. Services with volumes run one machine and stop the old one first. A volume records the image lineage that wrote it and refuses another without `allow_image_change`.
* Variables: `${{service.KEY}}`, `${{KEY}}` and `${{secret}}` resolve when a deploy starts, into the deployment's snapshot. Changes are dry-run resolved, dependents included, before anything is saved.
* `bird.toml` is one service; flags override it. Deploying never removes variables or domains set elsewhere, and a deploy's command is kept until `--default-command`.
* Builds run one at a time. They must name `outputformat`, or podman 5.8 never reuses cached layers (`tests/build.rs` guards this).
* Backups pause machines (crash-consistent) rather than stop them. With remote storage the export goes to local staging while paused and uploads after they resume; a restore downloads before the machines go and first saves the current data as a backup. Backups outlive `bird rm --purge`. An S3 upload that ends early, also by being dropped, is aborted. `bird.db` is copied into backup storage daily and found by listing, so the copies survive losing it.
* Cron: `[[cron]]` in bird.toml is sent as `DeployRequest.cron` only when deploying from the file, and replaces the service's jobs after the deploy succeeds (`None` keeps them). Schedules are 5-field UTC (croner); `last_due_at` is claimed in SQLite so a time runs once, times later than 2 minutes are dropped, and a stopped service records `skipped`. Runs are `bird run` containers with the job's timeout, never overlap per job, keep the last 64 KiB of output and 50 runs; shutdown and startup mark unfinished ones `interrupted`.
* One-off commands: `exec` uses podman's exec API, `run` a container removed however the session ends. Podman keeps an attached exec running when its client leaves, so birdd signals the exec's process group. Piped mode is bytes both ways; podman's own "connection reset by peer" notice is dropped when it ends the stream. `--no-entrypoint` runs without podman's init so podman itself reports a missing program.
* Machines otherwise run with podman's init as pid 1, so apps that ignore SIGTERM still stop.
* A service's `state` is desired state: `stop` keeps its containers, and any deploy starts a stopped service.
* Stats take two podman samples half a second apart, since podman's single-sample CPU is the average since start.
* Logs: `bird_podman::Lines` rejoins long lines podman splits; `follow` tracks the service, not fixed machines.
* The proxy sets `x-server: Bird` on every response (not configurable), caps request bodies and fails response bodies that go idle.
* Templates in `crates/bird-server/templates/` are embedded `bird.toml` manifests; pin their images to a major tag.

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
* Make invalid states unrepresentable: `Option`/enums over sentinel values, `NonZero*` over checked zeroes, a `Source::{Image, Build}` enum over two optional fields.
* Validation in a type's parser applies to rows already stored. Tighten rules for new input at the write path (e.g. `deploy()`), or old data stops loading.
* Prefer `let ... else` and `?` for early exits; match exhaustively on our own enums instead of `_`, so new variants are caught by the compiler.
* No `as` casts between numeric types; use `From`/`TryFrom` and decide what overflow means.
* Pass `&T`/`&[T]`/`impl Trait` in; return owned values or iterators. Take ownership only when storing or moving into a task.
* Constants with units in the name (`BUILD_TIMEOUT`, `MAX_CONTEXT_BYTES`) instead of magic numbers; config structs implement `Default`, tests override with `..Default::default()`.
* `#[must_use]` on pure functions and getters; `const fn` where it is free.
* Inspect wrapped errors with `std::iter::successors(Some(err), |&e| e.source())` and `is::<T>()`, never by matching error strings.
* `clippy.toml` allows unwrap/expect/panic/indexing only inside `#[test]` fns; helpers outside them (fixtures, integration test helpers) use `expect("why it holds")`.

## Async and concurrency

* `tokio` runtime everywhere.
* Never block inside async code. SQLite runs on a dedicated thread or via `spawn_blocking`.
* Every network call, Podman call, and health check has a timeout.
* Never hold a lock across `.await`. Prefer message passing or `arc-swap` for read-heavy shared state.
* Channels are bounded. Spawned tasks are tracked and shut down gracefully on SIGTERM.
* Anything tied to a connection or request (semaphore permits, shutdown watches) moves into every task that outlives it, or limits silently stop applying.
* A timeout on the first response is not enough: streams also need an idle timeout and a size cap.
* Custom `Body`/`Future` types stay `Unpin` (box the `Sleep`, require `B: Unpin`) so no pin projection is needed, and are generic over the inner body so they don't add another box.
* Cancellation is by drop: race work against shutdown and client disconnect with `tokio::select!` and make sure dropping mid-way leaves nothing half-done.

## Performance

* The proxy is the hot path: no per-request allocation beyond what is required, stream bodies and never buffer them fully, reuse upstream connections, route table read lock-free via `arc-swap`.
* Measure before optimizing. No premature caching or abstraction.
* Hot-path changes are benchmarked against the previous commit with `.e2e/bench.sh`, alternating old and new runs, and the cost is reported.
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
* A regression test must fail without the fix; test the behaviour from outside (real sockets, real Podman) rather than the implementation.
* A step is done when `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test -- --include-ignored` pass.

## Git

* One-line conventional commits, lowercase, imperative, no body, no attribution.
* Commit only when the user asks.
