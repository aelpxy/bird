---
name: bird
description: Deploy and run apps on a bird server with the `bird` cli. Use when asked to deploy, host, ship or put an app online with bird, add a database to it, set its variables or domains, read its logs, or find out why a deploy failed.
---

# Deploying with bird

bird is a self-hosted platform: `birdd` runs on a server, builds or pulls images, runs them as containers and routes domains to them with automatic HTTPS. You drive it with the `bird` cli. Everything below runs from your machine against that server.

The model is `project → environment → service`. A service is one app or database; its settings live in a `bird.toml` next to the code.

## 1. Connect

You need the server's address and an api token from the user. Never invent them, and never print or commit the token.

```sh
export BIRD_API=https://bird.example.com   # the dashboard domain, or host:port of the api
export BIRD_TOKEN=bird_...                 # from `bird token create <name>` on the server
bird whoami                                # check the connection
```

Or save the login once: `printf '%s' "$TOKEN" | bird login https://bird.example.com --token`.

If `bird` is missing, install the cli from the release (Linux or macOS, x86_64 or aarch64):

```sh
name="bird-$(uname -m | sed 's/arm64/aarch64/')-$(uname -s | sed 's/Darwin/macos/; s/Linux/linux/')"
curl -fsSL "https://github.com/aelpxy/bird/releases/download/edge/$name.tar.gz" | tar -xz
mkdir -p ~/.local/bin && install -m 0755 "$name/bird" ~/.local/bin/bird
```

## 2. Deploy an app

In the app's directory:

```sh
bird init                 # writes bird.toml: builds ./Dockerfile, or pass an image: bird init web nginx:alpine
bird deploy               # builds on the server or pulls, waits until healthy, exits non-zero on failure
bird status               # machines, cpu, memory
```

A typical `bird.toml` for an app built from source:

```toml
name = "web"
port = 3000                    # what the app listens on, on 0.0.0.0
domains = ["app.example.com"]
health = "/healthz"            # or "http" (any answer on /) or "tcp"
release = ["npm", "run", "migrate"]   # optional, runs before new machines start

[build]
context = "."
dockerfile = "Dockerfile"

[env]
NODE_ENV = "production"
DATABASE_URL = "${{postgres.DATABASE_URL}}"
```

Use either `image = "..."` or `[build]`, never both. Without either, deploy says "nothing to deploy". Every field is in [docs/bird-toml.md](../../docs/bird-toml.md); unknown fields are errors.

One-off deploys without a file:

```sh
bird deploy web ghcr.io/acme/web:1.4 --port 8080 --domain app.example.com
bird deploy api --build --port 3000
```

Rules that trip agents up:

- The app must listen on `0.0.0.0` and the configured `port`, not `localhost`.
- A Dockerfile is required for `[build]`; write one if the project has none, and add a `.dockerignore` for `node_modules`, `target`, `.git`.
- New traffic moves only once the new machines pass the health check, so a failed deploy leaves the old version serving.
- Deploying only adds variables and domains; it never removes ones set elsewhere.

## 3. Databases and other services

```sh
bird templates            # postgres, pgvector, mysql, mariadb, valkey, dragonfly, clickhouse, meilisearch
bird add postgres         # creates the service with a volume and a generated password
```

Link it from the app with a reference instead of copying credentials:

```sh
bird env set -s web 'DATABASE_URL=${{postgres.DATABASE_URL}}'
```

Templates expose `DATABASE_URL` (postgres, pgvector, mysql, mariadb), `VALKEY_URL`, `DRAGONFLY_URL`, `CLICKHOUSE_URL` and `MEILI_URL`. Services reach each other on the private network as `<service>` or `<service>.internal`; nothing is public unless it has a domain.

## 4. Variables and secrets

```sh
bird env                               # names only, values are never listed
bird env set KEY=value OTHER=value     # redeploys the service; --no-deploy only saves
bird env set 'SESSION_SECRET=${{secret}}'   # a random secret generated on the server
bird env unset KEY
```

- `${{service.KEY}}` reads another service's variable, `${{KEY}}` one of the same service.
- Quote values with `${{` in single quotes so the shell leaves them alone.
- Put secrets in `bird env set`, never in `bird.toml` or `[build].args`, and never print them back with `bird env get` unless the user asks.

## 5. Domains

```sh
bird domains add app.example.com
bird domains
```

The domain's DNS must point at the server (on Cloudflare, DNS only at first so the certificate can be issued). HTTPS follows automatically.

## 6. When something is wrong

```sh
bird deploy                 # its output already contains the build log and the new machines' output
bird history                # deployments, newest first, with their status
bird logs -n 200            # app logs; -f to follow
bird status --json
bird exec -T -- ls /app     # a command in a running machine; -T for plain output
bird run -T -- npm run seed # a fresh container with the service's image and variables
bird rollback               # back to the previous deployment
```

Common causes: the app listens on the wrong port or on `localhost`, the health path does not answer 2xx, a missing variable crashes the app at start (read `bird logs`), or a private image needs `bird registry login --username <user> <host>` with the password on stdin.

## 7. Other commands

- `bird scale 3`, `bird restart`, `bird stop`, `bird start`.
- `bird backup create`, `bird backup`, `bird backup restore <id>`; `[backup] every = "1d"` in bird.toml schedules them.
- `[[cron]]` entries in bird.toml run commands on a UTC schedule; `bird cron` lists them and `bird cron run <job>` runs one now.
- `bird connect -s postgres` tunnels a service's port to localhost for a local client.
- `bird project create <name>`, `bird environment create staging`, `bird switch <project> [environment]`.
- Read commands take `--json`. Every command has `--help`.

## Ground rules

- Service commands act on the service named in `./bird.toml`, or the one given with `-s <name>`.
- Ask before destructive commands: `bird rm` (with `--purge` it also deletes volume data), `bird backup restore`, `bird project remove`, `bird environment remove`. They refuse to run without a terminal unless given `-y`; pass `-y` only after the user agreed.
- Don't guess the server, token, project or domains; ask the user.
- To deploy on every push, use the GitHub Action in [docs/github-actions.md](../../docs/github-actions.md).
