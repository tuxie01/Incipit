#!/usr/bin/env bash
# Copy an existing project into legacy/ of an Incipit project, as a read-only
# reference. Build output and version-control data are left behind.
#
# Usage: import.sh <source-directory> [target-project]
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: import.sh <source-directory> [target-project]" >&2
  exit 64
fi
if [ ! -d "$1" ]; then
  echo "error: source '$1' is not a directory" >&2
  exit 1
fi

SRC="$(cd "$1" && pwd -P)"
TARGET="$(cd "${2:-.}" && pwd -P)"
DEST="$TARGET/legacy"

if [ ! -d "$TARGET/.incipit" ]; then
  echo "error: $TARGET is not an Incipit project (run the scaffold phase first)" >&2
  exit 1
fi
case "$SRC/" in "$TARGET"/*) echo "error: the source lies inside the target project" >&2; exit 1 ;; esac
case "$TARGET/" in "$SRC"/*) echo "error: the target project lies inside the source" >&2; exit 1 ;; esac
if [ -d "$DEST" ] && [ -n "$(ls -A "$DEST" 2>/dev/null)" ]; then
  echo "error: $DEST already holds an import. Remove it first to import again." >&2
  exit 1
fi

# Version-control data, dependencies and build output: reproducible or
# irrelevant for a reference copy, and often the bulk of the size.
EXCLUDES=".git .hg .svn node_modules target build dist out .venv venv __pycache__ .gradle .next .cache .DS_Store"

mkdir -p "$DEST"
if command -v rsync >/dev/null 2>&1; then
  ARGS=""
  for name in $EXCLUDES; do ARGS="$ARGS --exclude=$name"; done
  # shellcheck disable=SC2086
  rsync -a $ARGS "$SRC/" "$DEST/"
else
  ARGS=""
  for name in $EXCLUDES; do ARGS="$ARGS --exclude=$name"; done
  # shellcheck disable=SC2086
  tar -C "$SRC" $ARGS -cf - . | tar -C "$DEST" -xf -
fi

# Old agent instructions must not become live instructions in the new project:
# Claude Code loads CLAUDE.md and .claude/ from subdirectories it works in.
find "$DEST" -depth \( -name 'CLAUDE.md' -o -name 'CLAUDE.local.md' -o -name '.claude' -o -name '.mcp.json' -o -name 'AGENTS.md' \) | while IFS= read -r path; do
  dir="$(dirname "$path")"
  base="$(basename "$path")"
  mv "$path" "$dir/legacy-${base#.}"
  echo "renamed ${path#"$DEST"/} -> legacy-${base#.}"
done

FILES="$(find "$DEST" -type f | wc -l | tr -d ' ')"
SIZE="$(du -sh "$DEST" | awk '{print $1}')"

{
  echo "source=$SRC"
  echo "imported=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "files=$FILES"
  echo "excluded=$EXCLUDES"
} > "$TARGET/.incipit/import.txt"

# Read-only on disk as a second barrier next to the guard's role rules.
chmod -R a-w "$DEST" 2>/dev/null || true

echo "imported $FILES files ($SIZE) from $SRC to legacy/"
