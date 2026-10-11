---
name: bird
description: Deploys and runs apps on a self-hosted bird server with the `bird` CLI. Use when asked to deploy, host or ship an app with bird, write or fix a bird.toml, add a database, set variables or domains, read logs, or work out why a bird deploy failed.
---

# Deploying with bird

bird is a self-hosted platform. `birdd` runs on a server, builds or pulls images, runs them as containers and routes domains to them with automatic HTTPS. You drive it with the `bird` CLI from your machine.

The model is `org → project → environment → service`. A service is one app, worker or database. Its settings live in a `bird.toml` next to its code. A project holds every service a product needs, so an app and its database are two services in the same project and environment.

References in this folder:

- [reference/bird-toml.md](reference/bird-toml.md): every `bird.toml` field, with defaults and limits.
- [reference/github-actions.md](reference/github-actions.md): deploying on every push.

Every command also has `--help`.

## First deploy checklist

Copy this list and tick it off as you go:

```
- [ ] Connected: `bird whoami` works
- [ ] Project and environment chosen
- [ ] App listens on 0.0.0.0 and a known port
- [ ] Dockerfile builds (registry names on every FROM), or an image to run
- [ ] bird.toml written
- [ ] Databases added and referenced
- [ ] Variables, including build-time ones, passed with the first deploy
- [ ] Deployed, and the exit code is 0
- [ ] Checked: `bird status`, then the domain or `bird connect`
```

## 1. Connect

You need the server's address and an API token from the user. Never invent them, and never print or commit the token.

```sh
export BIRD_API=https://bird.example.com   # the dashboard domain, or host:port of the API
export BIRD_TOKEN=bird_...                 # from `bird token create <name>`
bird whoami                                # checks the connection
```

To save the login instead: `printf '%s' "$TOKEN" | bird login https://bird.example.com --token`.

If `bird` is missing, install the CLI (Linux or macOS, x86_64 or aarch64):

```sh
name="bird-$(uname -m | sed 's/arm64/aarch64/')-$(uname -s | sed 's/Darwin/macos/; s/Linux/linux/')"
curl -fsSL "https://github.com/aelpxy/bird/releases/download/edge/$name.tar.gz" | tar -xz
mkdir -p ~/.local/bin && install -m 0755 "$name/bird" ~/.local/bin/bird
```

## 2. Pick the project and environment

Commands act in the project and environment from `-p` and `-E`, then `project` and `environment` in bird.toml, then `bird switch`, then `default` and `production`. `BIRD_PROJECT` and `BIRD_ENVIRONMENT` work too.

```sh
bird project                       # projects and their environments
bird project create shop           # comes with a production environment
bird environment create staging -p shop
```

Put `project = "shop"` in bird.toml so every command from that folder lands in the right place. Ask the user which project to use rather than creating one unprompted.

## 3. Deploy an app

In the app's directory:

```sh
bird init                 # writes bird.toml that builds ./Dockerfile; or bird init web nginx:alpine
bird deploy               # builds on the server or pulls, waits until healthy
bird status               # machines, CPU and memory
```

A typical `bird.toml` for an app built from source:

```toml
name = "web"
project = "shop"
port = 3000                    # what the app listens on, on 0.0.0.0
domains = ["app.example.com"]
health = "/healthz"            # "http" (any answer on /, the default), "tcp", or a path that answers 2xx
release = ["npm", "run", "migrate"]   # optional, runs once before new machines start

[build]
context = "."
dockerfile = "Dockerfile"

[env]
NODE_ENV = "production"
DATABASE_URL = "${{postgres.DATABASE_URL}}"
```

Use either `image = "..."` or `[build]`, never both. Without either, deploy says "nothing to deploy". Unknown fields are errors. Every field is in [reference/bird-toml.md](reference/bird-toml.md).

Deploys without a file, where flags do the job of bird.toml:

```sh
bird deploy web ghcr.io/acme/web:1.4 --port 8080 --domain app.example.com
bird deploy api --build --port 3000
bird deploy -c services/api/bird.toml   # a bird.toml somewhere else, like in a monorepo
```

Things that trip agents up:

- The app must listen on `0.0.0.0` and the configured `port`, not `localhost`. `bird init` reads the port from the Dockerfile's `EXPOSE`, else assumes 80. Pass `--port` when that is wrong.
- `[build]` needs a Dockerfile. Write one if the project has none, and a `.dockerignore` for `node_modules`, `target` and `.git`.
- Write every image in a Dockerfile with its registry, like `FROM docker.io/library/node:22` or `FROM docker.io/lukemathwalker/cargo-chef:latest-rust-1`. The server builds with Podman, which may refuse short names like `node:22`. Images given to `bird deploy` or `image =` can be short, since bird adds `docker.io/` itself.
- Traffic moves only once the new machines pass their health check, so a failed deploy leaves the old version serving.
- Deploying only adds variables and domains. It never removes ones set elsewhere.

### Variables on the first deploy

`bird env set` needs the service to exist. Don't deploy a broken version just to create it. Pass the variables with the first deploy:

```sh
bird deploy -e 'DATABASE_URL=${{postgres.DATABASE_URL}}' -e 'SESSION_SECRET=${{secret}}'
```

Or put them under `[env]` in bird.toml. Keep real secrets out of the file. Use `${{secret}}` for generated ones, or pass them with `-e`.

### Variables the build needs

Runtime variables don't exist while the image builds. Frameworks that bake values in at build time need build arguments instead: Next.js `NEXT_PUBLIC_*`, Vite `VITE_*`, Create React App `REACT_APP_*`.

```toml
[build]
args = { NEXT_PUBLIC_API_URL = "https://api.example.com" }
```

The Dockerfile declares each one before the step that reads it:

```dockerfile
ARG NEXT_PUBLIC_API_URL
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
RUN npm run build
```

`bird deploy --build-arg KEY=VALUE` does the same for one deploy. Build arguments stay readable in the image, so never put secrets in them.

### Private images

Save the registry login once. The password or token goes on stdin. Pulls and builds both use it.

```sh
printf '%s' "$GHCR_TOKEN" | bird registry login --username <user> ghcr.io
```

### Persistent data

```sh
bird deploy -v data:/var/lib/app      # or volumes = [{ name = "data", path = "/var/lib/app" }]
```

A service with volumes runs one machine, and its old machine stops before the new one starts, so its deploys have a short gap. A volume remembers the image that wrote it. Deploying a different image onto it needs `--allow-image-change`, so ask the user first.

### Reaching a service without a domain

A service without a domain is private. To test it:

- From your machine: `bird connect -s api` forwards its port to `localhost`. It runs until stopped, so start it in the background and stop it when done.
- Publicly before real DNS is ready: use a wildcard DNS name for the server's IP, like `--domain api.203-0-113-7.sslip.io` for a server at `203.0.113.7`. Ask the user for the IP. Remove it with `bird domains remove` once the real domain works.

## 4. Databases and other services

```sh
bird templates            # postgres, pgvector, mysql, mariadb, valkey, dragonfly, clickhouse, meilisearch
bird add postgres         # a service with a volume and a generated password; bird add postgres db names it db
```

Link it with a reference instead of copying credentials:

```sh
bird env set -s web 'DATABASE_URL=${{postgres.DATABASE_URL}}'
```

Templates expose `DATABASE_URL` (postgres, pgvector, mysql, mariadb), `VALKEY_URL`, `DRAGONFLY_URL`, `CLICKHOUSE_URL` and `MEILI_URL`. Services reach each other on the environment's private network as `<service>` or `<service>.internal`. Nothing is public unless it has a domain.

## 5. Variables and secrets

```sh
bird env                                    # names only, values are never listed
bird env set KEY=value OTHER=value          # redeploys the service; --no-deploy only saves
bird env set 'SESSION_SECRET=${{secret}}'   # a random secret generated on the server
bird env unset KEY
```

- `${{service.KEY}}` reads another service's variable, and `${{KEY}}` one of the same service.
- Quote anything with `${{` in single quotes so the shell leaves it alone.
- Never print a value back with `bird env get` unless the user asks.

## 6. Domains

```sh
bird domains add app.example.com
bird domains
```

The domain's DNS must point at the server. On Cloudflare, keep it DNS only until HTTPS works. HTTPS follows on its own.

## 7. When something is wrong

```sh
bird deploy                  # its output already has the build log and the new machines' output
bird history                 # deployments, newest first, with their status
bird logs -n 200             # the latest app logs
bird status --json
bird exec -T -- ls /app      # a command in a running machine
bird run -T -- npm run seed  # a fresh container with the service's image and variables
bird rollback                # back to the previous deployment
```

Common causes:

- The app listens on the wrong port, or on `localhost`.
- The health path doesn't answer 2xx. Use `--health http` or `tcp` if the app has no such path.
- A missing variable crashes the app at start. Read `bird logs`.
- A Dockerfile `FROM` without a registry, or a private image without `bird registry login`.

## 8. Other commands

- `bird scale 3`, `bird restart`, `bird stop`, `bird start`.
- `bird backup create`, `bird backup`, `bird backup restore <id>`. `[backup] every = "1d"` in bird.toml schedules them.
- `[[cron]]` entries in bird.toml run commands on a UTC schedule. `bird cron` lists them and `bird cron run <job>` runs one now.
- `drain`, `kill_signal` and `kill_timeout` in bird.toml control how old machines stop. Raise `kill_timeout` for apps that need longer to shut down.
- Rate limits, headers, maintenance mode and the Cloudflare setting are in the dashboard, under the service's proxy settings.

## Running commands as an agent

- Results go to stdout. Progress and errors go to stderr. Read commands take `--json` for parsing.
- A failed deploy or command exits non-zero. `bird exec`, `bird run` and `bird cron run` exit with the remote command's code.
- These never end on their own: `bird logs -f`, `bird status -w` and `bird connect`. Use `bird logs -n 200` instead, or run them in the background with a timeout.
- Always give `bird exec` and `bird run` a command, with `-T`. Without one they open an interactive shell.

## Ground rules

- Service commands act on the service named in `./bird.toml`, or the one given with `-s <name>`.
- Ask before destructive commands: `bird rm` (with `--purge` it also deletes the volume data), `bird backup restore`, `bird project remove`, `bird environment remove`. Without a terminal they refuse unless given `-y`. Pass `-y` only after the user agreed.
- Don't guess the server, token, project or domains. Ask the user.
- To deploy on every push, see [reference/github-actions.md](reference/github-actions.md).
