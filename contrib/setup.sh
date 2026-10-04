#!/usr/bin/env bash
# Installs or upgrades bird for the current user from this checkout, then starts it.
#
#   contrib/setup.sh                          build, install, start, log the CLI in
#   contrib/setup.sh --acme-email you@x.com   also get Let's Encrypt certificates
#   contrib/setup.sh --high-ports             proxy on :8080/:8443, no sudo needed
#   contrib/setup.sh --uninstall              stop and remove bird, keep its data
#
# Run it again to upgrade. It never runs as root itself and only asks for sudo to let your user
# bind ports 80 and 443.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin_dir="$HOME/.local/bin"
unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
drop_in="$unit_dir/birdd.service.d/setup.conf"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}/bird"
api="127.0.0.1:7070"
acme_email=""
high_ports=false
uninstall=false

if [[ -t 1 ]]; then
    bold=$'\e[1m' green=$'\e[32m' yellow=$'\e[33m' red=$'\e[31m' reset=$'\e[0m'
else
    bold="" green="" yellow="" red="" reset=""
fi
step() { printf '%s==>%s %s\n' "$bold" "$reset" "$*"; }
ok() { printf '%s✓%s %s\n' "$green" "$reset" "$*"; }
warn() { printf '%swarning:%s %s\n' "$yellow" "$reset" "$*" >&2; }
fail() {
    printf '%serror:%s %s\n' "$red" "$reset" "$*" >&2
    exit 1
}

usage() {
    sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while (($#)); do
    case "$1" in
        --acme-email)
            [[ $# -ge 2 ]] || fail "--acme-email needs an address"
            acme_email="$2"
            shift 2
            ;;
        --high-ports)
            high_ports=true
            shift
            ;;
        --uninstall)
            uninstall=true
            shift
            ;;
        -h | --help) usage 0 ;;
        *)
            warn "unknown option $1"
            usage 1
            ;;
    esac
done

[[ $EUID -ne 0 ]] || fail "run this as the user bird should run as, not root"
[[ "$(uname -s)" == Linux ]] || fail "bird runs on Linux only"
systemctl --user show-environment >/dev/null 2>&1 \
    || fail "no systemd user session; log in directly (not with su) or enable lingering first"

if $uninstall; then
    step "removing bird"
    systemctl --user disable --now birdd 2>/dev/null || true
    rm -f "$bin_dir/birdd" "$bin_dir/bird" "$unit_dir/birdd.service" "$drop_in"
    rmdir "$unit_dir/birdd.service.d" 2>/dev/null || true
    systemctl --user daemon-reload
    ok "bird is removed; its machines keep running until you remove them with podman"
    echo "its data is still in $data_dir; delete it to start over"
    exit 0
fi

step "checking requirements"
command -v podman >/dev/null || fail "podman is not installed (dnf install podman, or apt install podman)"
podman_version="$(podman version --format '{{.Client.Version}}')"
((${podman_version%%.*} >= 5)) || fail "bird needs podman 5 or newer, this is $podman_version"
if ! command -v cargo >/dev/null; then
    # rustup installs here without touching PATH for this shell
    [[ -x "$HOME/.cargo/bin/cargo" ]] || fail "rust is not installed: curl -sSf https://sh.rustup.rs | sh"
    PATH="$HOME/.cargo/bin:$PATH"
fi
ok "podman $podman_version, $(cargo --version)"

step "building bird"
(cd "$repo" && cargo build --release --quiet)
install -Dm755 "$repo/target/release/birdd" "$repo/target/release/bird" -t "$bin_dir"
ok "installed birdd and bird to $bin_dir"
case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) warn "$bin_dir is not on your PATH; add it to use the bird command" ;;
esac

step "turning on podman's api and lingering"
systemctl --user enable --now podman.socket >/dev/null
# without it, user services stop at logout and do not start at boot
if ! loginctl show-user "$USER" -p Linger 2>/dev/null | grep -q yes; then
    loginctl enable-linger "$USER" 2>/dev/null \
        || warn "could not turn on lingering; run: sudo loginctl enable-linger $USER"
fi
ok "podman.socket is running"

env_lines=()
if ! $high_ports; then
    if (($(sysctl -n net.ipv4.ip_unprivileged_port_start) > 80)); then
        step "letting your user bind ports 80 and 443 (needs sudo once)"
        if sudo sh -c 'echo net.ipv4.ip_unprivileged_port_start=80 > /etc/sysctl.d/90-bird.conf && sysctl -q --system'; then
            ok "ports 80 and 443 are bindable"
        else
            warn "no sudo, so the proxy listens on 8080 and 8443 instead"
            high_ports=true
        fi
    fi
fi
if $high_ports; then
    env_lines+=("BIRD_PROXY_ADDR=0.0.0.0:8080" "BIRD_HTTPS_ADDR=0.0.0.0:8443")
fi
if [[ -n $acme_email ]]; then
    $high_ports && warn "let's encrypt checks domains on port 80, forward it to 8080 or certificates fail"
    env_lines+=(
        "BIRD_ACME_DIRECTORY=https://acme-v02.api.letsencrypt.org/directory"
        "BIRD_ACME_EMAIL=$acme_email"
    )
fi

step "installing the birdd service"
install -Dm644 "$repo/contrib/systemd/birdd.service" -t "$unit_dir"
if ((${#env_lines[@]})); then
    mkdir -p "$(dirname "$drop_in")"
    {
        echo "# written by contrib/setup.sh, rerunning it replaces this file"
        echo "[Service]"
        printf 'Environment=%s\n' "${env_lines[@]}"
    } >"$drop_in"
else
    rm -f "$drop_in"
fi
systemctl --user daemon-reload
systemctl --user enable birdd >/dev/null 2>&1
systemctl --user restart birdd

# birdd writes its token on first start and answers once its api is up
for _ in $(seq 1 60); do
    if [[ -s "$data_dir/api-token" ]] && "$bin_dir/bird" login "$api" <"$data_dir/api-token" >/dev/null 2>&1; then
        break
    fi
    sleep 0.5
done
"$bin_dir/bird" whoami >/dev/null 2>&1 \
    || fail "birdd did not come up; see: journalctl --user -u birdd -e"
ok "birdd is running and the bird CLI is logged in as root"

proxy_port=80
$high_ports && proxy_port=8080
cat <<EOF

${bold}bird is ready.${reset}
  deploy something:  bird deploy web docker.io/library/nginx:alpine --domain web.localhost
  try it:            curl -H 'Host: web.localhost' http://127.0.0.1:$proxy_port
  logs:              journalctl --user -u birdd -f
  root token:        $data_dir/api-token (keep it private)
EOF
