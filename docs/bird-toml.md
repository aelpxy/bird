# bird.toml

`bird init` writes a starter file. With one in the directory, `bird deploy` needs no arguments. Every field:

```toml
name = "api"                       # the service, required
project = "shop"                   # where it deploys, see `bird project --help`
environment = "staging"

image = "ghcr.io/acme/api:1.4"     # the image to run, or use [build] below instead
port = 3000                        # what the app listens on
domains = ["api.example.com", "www.example.com"]
command = ["bin/server", "--workers", "4"]   # instead of the image's own command
release = ["bin/migrate"]          # runs once per deploy before new machines start

health = "/healthz"                # "http", "tcp" or a path that must answer 2xx
health_timeout = "2m"              # how long a new machine has to pass it
memory = "512m"                    # per machine, like "512m" or "2g"
cpus = 0.5                         # per machine, in cores
replicas = 2                       # machines to run

volumes = [{ name = "data", path = "/var/lib/app" }]

# [build]                          # or build from source, without an image line
# context = "."                    # relative to bird.toml
# dockerfile = "Dockerfile"
# args = { NODE_ENV = "production" }   # Dockerfile ARGs, readable in the image

[env]
DATABASE_URL = "${{postgres.DATABASE_URL}}"
LOG_LEVEL = "info"

[backup]                           # back up the volumes on a schedule
every = "1d"
keep = 7

[[cron]]                           # a command on a schedule, in UTC
name = "report"
schedule = "0 6 * * MON"           # 6:00 every Monday
command = ["bin/report", "--weekly"]
timeout = "30m"                    # stopped after this, default 1h

[[cron]]
name = "cleanup"
schedule = "@hourly"
command = ["bin/cleanup"]
```

| Field | Default | Notes |
| --- | --- | --- |
| `name` | required | Lowercase letters, digits and `-`, starting with a letter. |
| `project`, `environment` | your `bird switch`, else `default`/`production` | `-p` and `-E` win over them. |
| `image` | none | Use either `image` or `[build]`, not both. |
| `port` | `80` | The app must listen on `0.0.0.0`. |
| `domains` | none | Only added. Deploying never removes domains set elsewhere. |
| `command` | the image's | Kept for later deploys. `bird deploy --default-command` goes back to the image's. |
| `release` | none | Runs in a fresh container with the new image and variables but no volumes, before any new machine starts. If it fails or runs over 10 minutes, the deploy fails and the old machines keep serving. Kept for later deploys and rollbacks; `bird deploy --no-release` drops it. |
| `health` | `"http"` (any answer on `/`) | `"tcp"` only waits for the port to accept connections. |
| `health_timeout` | `60s` | `"90s"`, `"2m"` or a number of seconds, up to an hour. |
| `memory` | `1g` | 32 MiB to 256 GiB. |
| `cpus` | `1` | 0.1 to 64. |
| `replicas` | `1` | Up to 32. A service with volumes runs exactly one. |
| `volumes` | none | Kept across deploys. Only `bird rm --purge` deletes them. |
| `[build]` | none | `context` defaults to the file's directory, `dockerfile` to `Dockerfile`. `.dockerignore` is honored. |
| `[env]` | none | Only added. Keep secrets out of the file and set them with `bird env set`. |
| `[backup]` | none | `every` from `1h` to `30d`. `keep` from 1 to 1000, default 7. |
| `[[cron]]` | none | The file's jobs replace the service's once a deploy works. Deploys without the file keep them. |

One file is one service. Flags win over the file, and flags you can repeat, like `--domain`, add to its lists. Unknown fields are errors, so typos fail instead of being ignored.

