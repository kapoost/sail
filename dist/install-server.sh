#!/bin/sh
# Put the relay on a Linux machine, as a service, without disturbing anything else on it.
#
#   curl -fsSL .../install-server.sh | sudo sh          # first time
#   sudo sh install-server.sh                           # and to upgrade
#
# What it does, in full, because a script that runs as root should be readable in one
# sitting:
#
#   * downloads the latest sail-server into /usr/local/bin
#   * writes /etc/sail-server.env with a port, a seed and a word, if it is not there
#   * installs a systemd unit, enables it and starts it
#
# What it deliberately does not do:
#
#   * touch the firewall. It prints the one line you need and lets you look at it first:
#     a script that edits firewall rules on a machine it knows nothing about is a script
#     that can lock somebody out of their own server, and this one is being run on a box
#     that has other things on it.
#   * overwrite the env file. Changing the word or the seed under a game that is running
#     would put everybody in a different sea and lock them out of it.
#   * run anything as root that does not have to be. The service itself runs as a
#     DynamicUser with no home, no shell and no privileges.

set -eu

BIN=/usr/local/bin/sail-server
ENV=/etc/sail-server.env
UNIT=/etc/systemd/system/sail-server.service
REPO=https://github.com/kapoost/sail
PORT=${SAIL_PORT:-7714}

if [ "$(id -u)" != "0" ]; then
  echo "run this with sudo: it writes to /usr/local/bin and /etc/systemd" >&2
  exit 1
fi
command -v systemctl >/dev/null || { echo "no systemd here" >&2; exit 1; }

# The binary. From the release, which is built on a runner rather than on somebody's
# laptop — see .github/workflows/release.yml.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "fetching the latest sail-server"
curl -fsSL "$REPO/releases/latest/download/sail-server" -o "$tmp/sail-server" 2>/dev/null ||
  { echo "no linux build published yet — build it with:"; \
    echo "  cargo build --release -p sail-net --bin sail-server"; \
    echo "and copy target/release/sail-server to $BIN yourself"; exit 1; }
install -m 0755 "$tmp/sail-server" "$BIN"
echo "installed $BIN"

# The settings, once. A word out of the kernel's own randomness, in the shape of
# something a person can read down a telephone.
if [ ! -f "$ENV" ]; then
  word=$(tr -dc 'a-z' < /dev/urandom | head -c 4)-$(tr -dc 'a-z' < /dev/urandom | head -c 5)
  seed=$(od -An -N4 -tu4 < /dev/urandom | tr -d ' ')
  umask 077
  cat > "$ENV" <<ENVFILE
# The relay's settings. Read by systemd, never by anybody else: this file is 0600.
SAIL_PORT=$PORT
# A fixed seed, so a restart puts everybody back in the same archipelago. Change it for
# a different sea, and tell everybody before you do.
SAIL_SEED=$seed
# The word a stranger does not have. Tell your friends; it is a doorbell, not a password,
# and it travels over the wire in the clear.
SAIL_WORD=$word
ENVFILE
  chmod 600 "$ENV"
  echo "wrote $ENV"
else
  echo "keeping the $ENV you already have"
fi

install -m 0644 "$(dirname "$0")/sail-server.service" "$UNIT" 2>/dev/null ||
  curl -fsSL "$REPO/raw/main/dist/sail-server.service" -o "$UNIT"
chmod 0644 "$UNIT"
systemctl daemon-reload
systemctl enable --now sail-server
sleep 1
systemctl --no-pager --lines=5 status sail-server || true

port=$(sed -n 's/^SAIL_PORT=//p' "$ENV")
cat <<DONE

The relay is up on UDP $port. Two things left, and neither of them is automatic:

  1. Let the port through. On a Hetzner Cloud machine that is the Cloud Firewall in the
     console as well as whatever is on the box itself:

         ufw allow $port/udp comment 'sail'        # if you use ufw

  2. Read the word out to whoever is sailing:

         sudo sed -n 's/^SAIL_WORD=//p' $ENV

Everybody else puts the address and the word in the TOGETHER page. To see what it is
doing: journalctl -u sail-server -f
DONE
