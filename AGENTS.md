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
* Data model: `project → environment → service → deployment → machine`, plus `domains` and `variables` on services.
* Users: `users` (role `admin` or `member`) own `api_tokens`, stored only as the SHA-256 of the secret (`bird_` plus 40 random characters, shown once on creation; the first 12 characters are kept to tell tokens apart) with an optional expiry. `api::auth::authenticate` resolves the bearer token to a `Principal`: the token in birdd's data dir is `Principal::Root`, an admin that cannot be deleted so nobody is locked out, anything else is looked up by hash (expired tokens fail). It records use at most once a minute off the request path and runs the request in a span with the user's name, so every log line says who acted. Admins manage users and everyone's tokens, members their own; everything else is open to any user until orgs scope it. CLI: `bird whoami`, `bird user`, `bird token [--user]`; new secrets go to stdout and everything else to stderr, so `bird token create ci > ci-token` works.
* Projects and environments: service routes live under `/v1/projects/{project}/environments/{environment}/...`; `api::scope::{Scope, ServiceScope}` resolve the path to an `EnvironmentId` (404 when missing) and everything below takes it explicitly (`state.service(environment, name)`, the deploy ticket is keyed by environment and name). `environments.rs` creates a project with a `production` environment, and deletes environments or projects only when no services or backups are left, checked in the same transaction as the delete. Each environment has its own Podman network (`environments.network`, `bird_<project>_<environment>`, created with the environment and removed with it; every network bird creates has podman's `isolate=strict`, since bridges otherwise route into each other by address, while published ports, which the proxy and health checks use, keep working); NULL means the `--network` one, which `default/production` from before environments existed keeps. Variable references, network aliases and backups stay inside an environment; domains, registries and built images are global. The CLI acts in `-p`/`-E` (`BIRD_PROJECT`/`BIRD_ENVIRONMENT`), then `project`/`environment` in bird.toml, then what `bird switch` saved in the login profile (its environment only for its project), then `default/production`, and builds paths with `ApiClient::scoped`.
* Private network: every machine joins its environment's Podman network with aliases `<service>` and `<service>.internal`, so services call each other directly by name. DNS returns every replica's address in a fixed order, so spreading internal traffic across replicas is up to the client.
* Volumes: a service with volumes runs exactly one machine and deploys by stopping the old machine (state `Stopped`, kept for a restore if the new one fails) before starting the new one. Each volume records the image lineage (repository, major version, variant) that first used it and refuses another without `allow_image_change`. Images bird builds (`localhost/...`) are one lineage per repository, since their tag is a timestamp; older records that still carry it as a major version match too. Volumes are only deleted by `bird rm --purge`.
* Variable references: values may contain `${{service.KEY}}`, `${{KEY}}` (same service) and `${{secret}}`/`${{secret(N)}}`. Secrets are generated once when set and stored as plain values; references are stored as written and resolved when a deploy starts, into the deployment snapshot's `resolved` column that machines run with. Changes are dry-run resolved, including services that depend on the changed one, before anything is saved.
* `bird.toml`: one service per file, parsed by the CLI into `bird_api::Manifest` and sent as a normal deploy; flags override it and repeatable flags add to its lists. Deploying from it only adds variables and domains, it never removes ones set elsewhere. A deploy's command is saved with the service and kept by later deploys that give none; `--default-command` (`default_command`) clears it so the image's own runs again. Unknown fields are errors. Secrets stay out of it (`bird env set`).
* Builds: `[build]` in `bird.toml` or `bird deploy --build [dir]` packs the context as a gzipped tar (trimmed by `.dockerignore`, which podman applies again) and uploads it to `POST /v1/services/{name}/builds`; birdd builds it with podman and streams `BuildEvent`s back. Builds run one at a time (others queue) and take `[build] args` / `--build-arg` as Dockerfile ARGs, which stay readable in the image. Built images are tagged `localhost/bird/<service>:<millis>`, are never pulled, and the last few distinct images per service are kept for rollbacks. The build request names `outputformat` (OCI): podman 5.8's build API stores layers either way but only reuses them when the format is given, and `tests/build.rs` fails if a rebuild stops hitting the cache.
* Backups: `bird backup create` pauses the service's running machines (crash-consistent, no restart) while podman exports each volume as a tar into backup storage, then records it in SQLite keyed by service name, so backups outlive `bird rm --purge`. Restore first saves the current data as a `restore` backup, destroys the machines, recreates each volume empty (podman's import only adds files), imports, and launches the active deployment again; it refuses data from another image lineage without `allow_image_change`. Both hold the service's deploy ticket so the supervisor stays out; it unpauses machines a crash left paused. Storage adapters implement `backups::storage::Adapter` (`put`/`get`/`exists`/`delete` on keys like `<backup id>/<volume>.tar`) and are a variant of `BackupStorage`; `LocalDir` (`--backup-dir`, default `<data>/backups`) and `S3` (`--s3-bucket` with `--s3-endpoint`, `--s3-region`, `--s3-prefix` and `BIRD_S3_ACCESS_KEY_ID`/`BIRD_S3_SECRET_ACCESS_KEY`, for any S3-compatible service) exist. `S3` wraps `object_store` (features `aws-base` and `ring`, reqwest with `rustls-no-provider`, so ring is installed as rustls' provider and no aws-lc or OpenSSL is built); it streams multipart uploads in 16 MiB parts, which only appear once complete, with connect and per-read timeouts instead of a whole-request one. Remote storage is slow, so `BackupStorage::staging()` gives a local `LocalDir::scratch` (`<data>/backup-staging`, no fsync, wiped at startup): a backup exports into it while machines are paused and uploads after they resume, and a restore downloads into it before the machines go. Adapters also `list` keys under a prefix. Tests run against SeaweedFS (MinIO no longer publishes images) started with podman, with signatures checked. `backups::Scheduler` ticks every minute: services with a `backup_schedules` row (`bird backup schedule` or `[backup]` in bird.toml) get a `scheduled` backup when the last one is older than `every`, taking the deploy ticket only if free (busy services wait for the next tick, failures retry after 15 minutes), then only `scheduled` backups beyond `keep` are deleted. It also copies `bird.db` with `VACUUM INTO` to `database/bird-<millis>.db` in backup storage (default daily, keep 7); those are found by listing storage, not through bird.db, so they survive losing it.
* Machine stats: `GET /v1/services/{name}?stats=true` (what `bird status` asks for) takes two podman stats samples half a second apart, because podman's own CPU figure with `stream=false` is the average since the container started. CPU is reported in millicores and shown as a share of the service's limit; memory and network are the latest sample. Failing to sample only logs; the listing never samples.
* Health checks: `http` (any HTTP response on `/`), `tcp`, or a path that must answer 2xx, sent with `Host: localhost`. The deploy waits `health_timeout` (default 60s) for the first pass and the supervisor keeps probing with the same check. A path check is stored as `health = 'http'` plus `health_path`, since the column's CHECK predates paths.
* Logs: podman's log driver splits long lines into frames without a newline, which `bird_podman::Lines` joins back. `follow=true` follows the service rather than fixed machines: it checks every second for running machines it is not following (deploy replacements, `start`), shows a new one from its start and a restarted one from where its stream ended, and ends only when the client leaves, birdd stops or the service is removed.
* One-off commands: `bird exec` runs in a running machine through podman's exec API (podman cannot stop an exec, so a disconnect only stops reading). `bird run` creates a `Lifecycle::OneOff` container (no published port, no restart, no volumes, no environment label so the orphan sweep skips it, `bird.run` label instead) from the active deployment's image, or the newest when none succeeded, with that deployment's resolved variables; it is removed on exit, disconnect, timeout or shutdown, and leftovers are removed at startup. Like `docker run`, the command goes to the image's entrypoint as arguments; `--no-entrypoint` (`skip_entrypoint` in the API) sets the entrypoint to its first word and drops the image's command, which podman does too once an entrypoint is given. Both stream `CommandEvent`s ending in `exited` or `failed` (what the CLI uses with `--json`). Output events are chunks exactly as written (`OutputFollower`, not the line-splitting `LogFollower`), keeping characters split across podman frames whole; bytes that are not UTF-8 become U+FFFD. Without `--json` and without a terminal (or with `-T`) the CLI uses `GET /v1/services/{name}/{exec,run}/pipe` instead, upgraded to `bird-tty` like the terminals below but with `commands::tty::Stdio::Piped`: podman's exec or a `Lifecycle::Piped` container (stdin open, no terminal) is attached, birdd splits podman's stdout and stderr frames with `bird_podman::Demux` as bytes arrive into data and stderr frames, and the client's eof frame half-closes podman's side, which the command reads as end of file. A command that exits with input unread makes podman reset the connection (and write its own error to stderr); birdd treats that as the end of output, stops forwarding input it cannot write, and drains the client after the exit frame so it is not lost to a reset. Everything is bytes, so `pg_dump -Fc > file` and `pg_restore < file` work; the CLI passes stdin on unless it is a terminal (then the command gets end of file at once, like `docker exec` without `-i`). The CLI exits with the remote code. From a terminal (stdin and stdout both ttys, no `--json` or `-T`) `bird exec` instead upgrades `GET /v1/services/{name}/exec/tty` to `bird-tty` and speaks `bird_api::Frame`s (data both ways, resize from the client, exit or error from birdd), with stdin read on a plain thread and the terminal in raw mode (rustix, restored on drop); birdd relays them to podman's hijacked tty exec. Interactive `bird run` uses `GET /v1/services/{name}/run/tty`: a `Lifecycle::Terminal` container (terminal and stdin open) is attached before it starts, then removed when the session ends however it ends; attach sends no request body, since podman feeds it to the container's stdin. `commands::tty::Remote::{Exec, Container}` holds what differs (resize, exit code, ending). Podman keeps an attached exec (tty or piped) running when its client leaves, so birdd sends its process group (podman makes each exec a group leader) SIGHUP then SIGKILL (it runs as birdd's user); each session holds a `terminals` permit (max 64) until then, and shutdown waits for every permit.
* Stop/start/restart: a service's `state` (`running`/`stopped`) is desired state. `stop` marks it stopped, records its machines `Stopped` before stopping the containers (so the proxy drops them first) and keeps the containers, so logs survive and `start` boots the same containers; the supervisor skips stopped services and startup recovery leaves their stopped machines alone. Any deploy (and rollback, variable redeploy) starts a stopped service, a failed one puts it back; restore leaves a stopped service stopped. `restart` stops and boots running machines one at a time behind the deploy ticket and is refused while stopped.
* Machines run with podman's init as pid 1, so apps that ignore SIGTERM still stop promptly.
* Proxy: every response it produces carries `x-server: Bird`, replacing the app's and not configurable. Request bodies are capped (`max_body_bytes`, 413), response bodies fail after `body_idle_timeout` without data, and websocket tunnels keep their connection's slot. An absolute request target wins over `Host`, and the app receives that host.
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
