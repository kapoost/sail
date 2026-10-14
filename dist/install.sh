#!/usr/bin/env bash
#
# Install Sail on a Mac, from one line pasted into Terminal:
#
#     curl -fsSL https://raw.githubusercontent.com/kapoost/sail/main/dist/install.sh | sh
#
# # Why this exists, which is not "because an installer is nice"
#
# A Mac refuses to open an application that nobody has paid Apple to vouch for. The
# refusal is not a warning you click past: it is "Sail is damaged and can't be opened",
# and the file is not damaged. For years the way past it was right-click and Open, which
# is what the release notes said — and in macOS 15 Apple took that away. Since then the
# only route is System Settings, Privacy & Security, scroll down, Open Anyway, and
# nobody finds that on their own. Reported from the deck: a friend who could not open the
# game at all and ended up building it from source to sail.
#
# What makes this script a fix rather than a workaround is a detail of how quarantine
# works: the attribute is put on by *the program that downloads the file*, not by the
# system. A browser marks what it saves; `curl` does not. So an application that arrives
# through here has no quarantine on it, and it opens by double-clicking like anything
# else — no dialogue, no Settings, nothing to explain to anybody.
#
# Which is also why the installer is a line you paste rather than a thing you download:
# an installer saved from a web page would be quarantined itself, and we would be back
# where we started one step further along.
#
# The real fix is a Developer ID and notarisation, 99 dollars a year, after which a file
# from a web page simply works. This is what you do before that is worth it.

set -eu

REPO="kapoost/sail"
APPS="$HOME/Applications"
APP="$APPS/Sail.app"
SERVER="$APPS/sail-server"

say() { printf '%s\n' "$*"; }
oops() { printf '%s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || oops "This installer is for macOS. On Windows use install.ps1."

# macOS 11 is what the bundle declares it needs.
major="$(sw_vers -productVersion | cut -d. -f1)"
[ "$major" -ge 11 ] 2>/dev/null || oops "Sail wants macOS 11 or newer; this is $(sw_vers -productVersion)."

say "Looking for the latest release of $REPO..."
# The release API rather than a fixed URL, so this script does not have to be edited
# every time a version goes out — which is the version of this that rots.
api="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest")" \
  || oops "Could not reach GitHub. Are you online?"
tag="$(printf '%s' "$api" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)"
url="$(printf '%s' "$api" | tr ',' '\n' | sed -n 's/.*"browser_download_url": *"\([^"]*Sail-macos\.zip\)".*/\1/p' | head -1)"
[ -n "$url" ] || oops "That release has no macOS build in it yet. Try again in an hour."
say "Found $tag."

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
say "Downloading..."
curl -fL --progress-bar -o "$tmp/sail.zip" "$url" || oops "The download failed."

say "Unpacking..."
unzip -q "$tmp/sail.zip" -d "$tmp/out" || oops "The archive would not open."
[ -d "$tmp/out/Sail.app" ] || oops "There is no Sail.app in that archive."

mkdir -p "$APPS"
# Only ever inside ~/Applications, and only ever these two names.
rm -rf "$APP"
mv "$tmp/out/Sail.app" "$APP"
if [ -f "$tmp/out/sail-server" ]; then
  rm -f "$SERVER"
  mv "$tmp/out/sail-server" "$SERVER"
  chmod +x "$SERVER"
fi

# Belt and braces. Nothing that came through curl is quarantined, but a copy the person
# already downloaded with a browser may have left the attribute on something here, and
# one quarantined file inside a bundle is enough to have the whole thing refused.
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true
[ -f "$SERVER" ] && xattr -dr com.apple.quarantine "$SERVER" 2>/dev/null || true

# The bundle is signed ad-hoc in CI, which is what an Apple Silicon Mac needs in order to
# run it at all. If that signature did not survive the trip, put one back: no certificate,
# no account, and enough to satisfy the kernel.
if ! codesign --verify --strict "$APP" >/dev/null 2>&1; then
  say "Re-signing..."
  codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
fi

say ""
say "Sail $tag is in ~/Applications. Double-click it, or:"
say "    open $APP"
if [ -f "$SERVER" ]; then
  say ""
  say "To host a game for your friends:"
  say "    $SERVER --seed 42"
  say "and they join from the game's own menu: Esc, then TOGETHER."
fi
say ""
say "Run this line again any time to get the newest version."
