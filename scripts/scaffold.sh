#!/usr/bin/env bash
# Copy the project template into a target directory. Existing files are kept,
# so the script is safe to run again.
#
# Usage: scaffold.sh [target-directory]
# Env:   INCIPIT_TEMPLATE  template name below templates/ (default: base)
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMPLATE="${INCIPIT_TEMPLATE:-base}"
SRC="$PLUGIN_ROOT/templates/$TEMPLATE"
BIN="$HOME/.incipit/bin/incipit"

if [ ! -d "$SRC" ]; then
  echo "error: template '$TEMPLATE' not found in $PLUGIN_ROOT/templates" >&2
  exit 1
fi
if [ ! -x "$BIN" ]; then
  echo "error: $BIN is missing. Run scripts/install-server.sh first." >&2
  exit 1
fi

mkdir -p "${1:-.}"
TARGET="$(cd "${1:-.}" && pwd)"

# Template names avoid leading dots so that nothing in the plugin itself is
# picked up as live configuration. Map them to their real names here.
real_name() {
  printf '%s\n' "$1" | sed \
    -e 's#^dot-claude/#.claude/#' \
    -e 's#^dot-incipit/#.incipit/#' \
    -e 's#^dot-mcp\.json$#.mcp.json#' \
    -e 's#^gitignore$#.gitignore#'
}

(cd "$SRC" && find . -type f | sed 's#^\./##' | sort) | while IFS= read -r rel; do
  dest="$TARGET/$(real_name "$rel")"
  shown="$(real_name "$rel")"
  if [ "$shown" = ".gitignore" ] && [ -f "$dest" ]; then
    # Merge instead of skip: add the lines that are not there yet.
    added=0
    while IFS= read -r line; do
      if [ -n "$line" ] && ! grep -qxF "$line" "$dest"; then
        printf '%s\n' "$line" >> "$dest"
        added=1
      fi
    done < "$SRC/$rel"
    if [ "$added" = 1 ]; then echo "merged  $shown"; else echo "kept    $shown"; fi
  elif [ -e "$dest" ]; then
    echo "kept    $shown"
  else
    mkdir -p "$(dirname "$dest")"
    cp "$SRC/$rel" "$dest"
    echo "created $shown"
  fi
done

for dir in .incipit/tasks .incipit/evidence docs scripts; do
  mkdir -p "$TARGET/$dir"
done
[ -e "$TARGET/.incipit/evidence/.gitkeep" ] || : > "$TARGET/.incipit/evidence/.gitkeep"
[ -e "$TARGET/.incipit/tasks/.gitkeep" ] || : > "$TARGET/.incipit/tasks/.gitkeep"

if command -v git >/dev/null 2>&1 && ! git -C "$TARGET" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git -C "$TARGET" init -q
  echo "created git repository"
fi

INCIPIT_PROJECT="$TARGET" "$BIN" phase done scaffold
