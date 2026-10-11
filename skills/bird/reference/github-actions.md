# Deploying from GitHub Actions

The `aelpxy/bird` action installs the `bird` cli and runs `bird deploy` in your repository, so every
push builds and deploys it like you would from your own machine.

## Set up the server once

The runner reaches birdd over https, through the dashboard domain. Install or reinstall with one:

```sh
curl -fsSL https://raw.githubusercontent.com/aelpxy/bird/main/install.sh | bash -s -- \
  --acme-email you@example.com --dashboard-domain bird.example.com
```

Point `bird.example.com` at the server, then make a token for the workflow:

```sh
bird token create github-actions
```

Save it as a repository secret named `BIRD_TOKEN` (Settings → Secrets and variables → Actions).

## The workflow

`.github/workflows/deploy.yml`, next to a `bird.toml` with `[build]` (or an `image`):

```yaml
name: deploy
on:
  push:
    branches: [main]

concurrency: deploy

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: aelpxy/bird@main
        with:
          api: https://bird.example.com
          token: ${{ secrets.BIRD_TOKEN }}
```

The build runs on your server, not the runner; the repository is uploaded as the build context,
honouring `.dockerignore`. Traffic moves to the new machines once they pass their health check, and
a failed deploy fails the job while the previous deployment keeps serving.

## Inputs

| Input | Default | |
| --- | --- | --- |
| `api` | required | `https://` and the dashboard domain |
| `token` | required | an api token, from a secret |
| `path` | `.` | directory holding `bird.toml` |
| `service` | from `bird.toml` | service to deploy |
| `project` | from `bird.toml`, then `default` | |
| `environment` | from `bird.toml`, then `production` | |
| `args` | none | more `bird deploy` arguments, like `--env KEY=value` |
| `version` | `edge` | which bird release's cli to use |

Linux runners only, x86_64 or arm64. The same works from any CI: set `BIRD_API` and `BIRD_TOKEN`
and run `bird deploy`.
