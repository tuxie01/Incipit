#!/usr/bin/env bash
# Install the incipit binary (ticket server, guard, owner CLI) to ~/.incipit/bin.
# Uses a prebuilt binary from server/dist/ if one matches this machine,
# otherwise builds from source with cargo.
#
# Usage: install-server.sh [--force]
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$HOME/.incipit/bin"
BIN="$BIN_DIR/incipit"
FORCE="${1:-}"

WANT="$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$PLUGIN_ROOT/server/Cargo.toml" | head -n 1)"

if [ "$FORCE" != "--force" ] && [ -x "$BIN" ]; then
  HAVE="$("$BIN" version 2>/dev/null | awk '{print $2}' || true)"
  if [ "$HAVE" = "$WANT" ]; then
    echo "incipit $HAVE already installed at $BIN"
    exit 0
  fi
fi

mkdir -p "$BIN_DIR"
PREBUILT="$PLUGIN_ROOT/server/dist/incipit-$(uname -s)-$(uname -m)"

if [ -f "$PREBUILT" ]; then
  SOURCE="$PREBUILT"
  echo "using prebuilt binary $(basename "$PREBUILT")"
elif command -v cargo >/dev/null 2>&1; then
  echo "building incipit $WANT from source"
  # Build outside the plugin directory: it may be a read-only cache.
  cargo build --release --locked \
    --manifest-path "$PLUGIN_ROOT/server/Cargo.toml" \
    --target-dir "$HOME/.incipit/build"
  SOURCE="$HOME/.incipit/build/release/incipit"
else
  echo "error: no prebuilt binary for $(uname -s)-$(uname -m) in server/dist/ and cargo is not installed." >&2
  echo "Install Rust (https://rustup.rs) or place a prebuilt binary at:" >&2
  echo "  $PREBUILT" >&2
  exit 1
fi

# Copy next to the target, then rename: never leaves a half-written binary.
cp "$SOURCE" "$BIN.new"
chmod 755 "$BIN.new"
mv -f "$BIN.new" "$BIN"
echo "installed $("$BIN" version) at $BIN"
