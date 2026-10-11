#!/usr/bin/env bash
# Installs or upgrades bird for the current user, then starts it.
#
#   curl -fsSL https://raw.githubusercontent.com/aelpxy/bird/main/install.sh | bash
#   curl -fsSL .../install.sh | bash -s -- --acme-email you@x.com
#
#   --acme-email <email>   also get Let's Encrypt certificates
#   --high-ports           proxy on :8080/:8443, no sudo needed
#   --version <tag>        install this release instead of edge (the latest commit on main)
#   --uninstall            stop and remove bird, keep its data
#
# From a checkout (./install.sh) it builds bird from source instead of downloading it.
# Run it again to upgrade. Run as root, it creates a `bird` user and runs bird as that user;
# otherwise it only asks for sudo to install podman and to let your user bind ports 80 and 443.
set -euo pipefail

github="https://github.com/aelpxy/bird"
script_url="https://raw.githubusercontent.com/aelpxy/bird/main/install.sh"
service_user="bird"
args=("$@")
bin_dir="$HOME/.local/bin"
unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
drop_in="$unit_dir/birdd.service.d/setup.conf"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}/bird"
api="127.0.0.1:7070"
acme_email=""
high_ports=false
uninstall=false
version=""

# piped from curl there is no file, so no checkout to build from
repo=""
script="${BASH_SOURCE[0]:-}"
if [[ -f $script && -f "$(dirname "$script")/crates/bird-server/Cargo.toml" ]]; then
    repo="$(cd "$(dirname "$script")" && pwd)"
fi

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
    cat <<'EOF'
usage: install.sh [--acme-email <email>] [--high-ports] [--version <tag>] [--uninstall]

  --acme-email <email>   also get Let's Encrypt certificates
  --high-ports           proxy on :8080/:8443, no sudo needed
  --version <tag>        install this release instead of edge (the latest commit on main)
  --uninstall            stop and remove bird, keep its data
EOF
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
        --version)
            [[ $# -ge 2 ]] || fail "--version needs a release tag, like v0.1.0"
            version="$2"
            shift 2
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

[[ "$(uname -s)" == Linux ]] || fail "bird runs on Linux only"
((EUID == 0)) || systemctl --user show-environment >/dev/null 2>&1 \
    || fail "no systemd user session; log in directly (not with su) or enable lingering first"

as_root() {
    if ((EUID == 0)); then
        "$@"
    else
        sudo "$@"
    fi
}

if $uninstall && ((EUID != 0)); then
    step "removing bird"
    systemctl --user disable --now birdd 2>/dev/null || true
    rm -f "$bin_dir/birdd" "$bin_dir/bird" "$unit_dir/birdd.service" "$drop_in"
    rmdir "$unit_dir/birdd.service.d" 2>/dev/null || true
    systemctl --user daemon-reload
    ok "bird is removed; its machines keep running until you remove them with podman"
    echo "its data is still in $data_dir; delete it to start over"
    exit 0
fi

install_podman() {
    local install
    if command -v dnf >/dev/null; then
        install=(dnf install -y podman)
    elif command -v apt-get >/dev/null; then
        install=(sh -c 'apt-get update -q && apt-get install -y podman')
    elif command -v zypper >/dev/null; then
        install=(zypper --non-interactive install podman)
    elif command -v pacman >/dev/null; then
        install=(pacman -S --noconfirm --needed podman)
    else
        fail "podman is not installed and this distro's package manager is unknown; install podman 5 or newer"
    fi
    step "installing podman"
    as_root "${install[@]}" || fail "could not install podman; install podman 5 or newer yourself"
}

allow_low_ports() {
    as_root sh -c 'echo net.ipv4.ip_unprivileged_port_start=80 > /etc/sysctl.d/90-bird.conf && sysctl -q --system'
}

# rootless podman is the point, so root sets up a user and installs as them
install_as_service_user() {
    [[ -z $repo ]] || fail "from a checkout, run ./install.sh as the user bird should run as, not root"
    local home uid
    if ! id "$service_user" >/dev/null 2>&1; then
        step "creating the user $service_user to run bird"
        useradd --create-home --shell /bin/bash "$service_user"
    fi
    grep -q "^$service_user:" /etc/subuid \
        || usermod --add-subuids 100000-165535 --add-subgids 100000-165535 "$service_user"
    home="$(getent passwd "$service_user" | cut -d: -f6)"
    uid="$(id -u "$service_user")"
    if ! $uninstall && ! $high_ports && (($(sysctl -n net.ipv4.ip_unprivileged_port_start) > 80)); then
        allow_low_ports
    fi
    loginctl enable-linger "$service_user"
    for _ in $(seq 1 50); do
        [[ -S /run/user/$uid/bus ]] && break
        sleep 0.2
    done
    [[ -S /run/user/$uid/bus ]] || fail "the systemd user session of $service_user did not start"
    curl -fsSL "$script_url" | (cd / && runuser -u "$service_user" -- env \
        HOME="$home" USER="$service_user" LOGNAME="$service_user" \
        XDG_RUNTIME_DIR="/run/user/$uid" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" \
        bash -s -- "${args[@]}")
    if $uninstall; then
        rm -f /usr/local/bin/bird
        exit 0
    fi
    # so root can use the cli without switching users
    install -Dm755 "$home/.local/bin/bird" -t /usr/local/bin
    /usr/local/bin/bird login "$api" <"$home/.local/share/bird/api-token" >/dev/null
    ok "bird runs as $service_user; the bird command also works for root"
    exit 0
}

step "checking requirements"
command -v podman >/dev/null || install_podman
podman_version="$(podman version --format '{{.Client.Version}}')"
((${podman_version%%.*} >= 5)) \
    || fail "bird needs podman 5 or newer, this is $podman_version; upgrade your distro or its podman"
ok "podman $podman_version"
((EUID != 0)) || install_as_service_user

build_from_source() {
    if ! command -v cargo >/dev/null; then
        # rustup installs here without touching PATH for this shell
        [[ -x "$HOME/.cargo/bin/cargo" ]] || fail "rust is not installed: curl -sSf https://sh.rustup.rs | sh"
        PATH="$HOME/.cargo/bin:$PATH"
    fi
    step "building bird with $(cargo --version)"
    if command -v pnpm >/dev/null; then
        (cd "$repo/frontend" && pnpm install --frozen-lockfile --silent && pnpm build >/dev/null)
    else
        warn "pnpm is not installed, so birdd is built without its dashboard (https://pnpm.io/installation)"
    fi
    (cd "$repo" && cargo build --release --quiet)
    install -Dm755 "$repo/target/release/birdd" "$repo/target/release/bird" -t "$bin_dir"
    install -Dm644 "$repo/contrib/systemd/birdd.service" -t "$unit_dir"
}

download() {
    local arch tag="${version:-edge}" name
    case "$(uname -m)" in
        x86_64 | amd64) arch=x86_64 ;;
        aarch64 | arm64) arch=aarch64 ;;
        *) fail "no prebuilt bird for $(uname -m); build it from a checkout with ./install.sh" ;;
    esac
    for tool in curl tar sha256sum; do
        command -v "$tool" >/dev/null || fail "$tool is needed to download bird"
    done
    name="bird-$arch-linux"
    tmp="$(mktemp -d)"
    # global, so the trap still knows it once the function returns
    trap 'rm -rf "$tmp"' EXIT
    step "downloading bird $tag for $arch"
    curl -fsSL --retry 3 -o "$tmp/$name.tar.gz" "$github/releases/download/$tag/$name.tar.gz" \
        || fail "could not download $github/releases/download/$tag/$name.tar.gz"
    curl -fsSL --retry 3 -o "$tmp/$name.tar.gz.sha256" "$github/releases/download/$tag/$name.tar.gz.sha256" \
        || fail "could not download the checksum of $name.tar.gz"
    (cd "$tmp" && sha256sum --check --quiet "$name.tar.gz.sha256") || fail "the download does not match its checksum"
    tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
    install -Dm755 "$tmp/$name/birdd" "$tmp/$name/bird" -t "$bin_dir"
    install -Dm644 "$tmp/$name/birdd.service" -t "$unit_dir"
}

if [[ -n $repo && -z $version ]]; then
    build_from_source
else
    download
fi
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
        if allow_low_ports; then
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

step "starting the birdd service"
if ((${#env_lines[@]})); then
    mkdir -p "$(dirname "$drop_in")"
    {
        echo "# written by install.sh, rerunning it replaces this file"
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

# the dashboard signs in with a password, so the first install makes an admin to sign in as
admin="" password=""
if [[ "$("$bin_dir/bird" user list --json 2>/dev/null)" == "[]" ]]; then
    admin="$(printf '%s' "$USER" | tr 'A-Z' 'a-z' | tr -c 'a-z0-9-' '-' | cut -c1-63)"
    [[ $admin =~ ^[a-z][a-z0-9-]*$ ]] || admin="admin"
    password="$(head -c 18 /dev/urandom | base64 | tr -d '/+=')"
    if "$bin_dir/bird" user create "$admin" --admin >/dev/null \
        && printf '%s\n%s\n' "$password" "$password" | "$bin_dir/bird" user passwd --user "$admin" >/dev/null; then
        ok "created the dashboard admin $admin"
    else
        warn "could not create a dashboard admin; make one with: bird user create <name> --admin"
        admin=""
    fi
fi

proxy_port=80
$high_ports && proxy_port=8080
cat <<EOF

${bold}bird is ready.${reset}
  dashboard:         http://$api  (from another machine: ssh -L 7070:$api $USER@$(hostname))
EOF
if [[ -n $admin ]]; then
    cat <<EOF
  sign in as:        $admin / $password  (shown only now; change it under Account)
EOF
fi
cat <<EOF
  deploy something:  bird deploy web docker.io/library/nginx:alpine --domain web.localhost
  try it:            curl -H 'Host: web.localhost' http://127.0.0.1:$proxy_port
  logs:              journalctl --user -u birdd -f
  root token:        $data_dir/api-token (keep it private)
EOF
