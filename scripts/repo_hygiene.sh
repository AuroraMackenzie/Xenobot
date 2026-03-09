#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="$ROOT_DIR/target"
FRONTEND_NODE_MODULES="$ROOT_DIR/crates/web/frontend/node_modules"

APPLY=0
PRUNE_EMPTY_DIRS=0
REMOVE_TARGET=0
REMOVE_NODE_MODULES=0
STRICT_SOURCE=0

usage() {
  cat <<'EOF'
Usage:
  scripts/repo_hygiene.sh [--strict-source] [--apply] [--prune-empty-dirs] [--remove-target] [--remove-node-modules]

Default behavior:
  Audit-only (no deletion). Shows high-noise artifacts and disk usage hotspots.

Options:
  --strict-source        Fail when source tree contains .DS_Store files.
  --apply                Execute cleanup actions selected by flags.
  --prune-empty-dirs     Remove empty directories (excluding .git).
  --remove-target        Remove Cargo target directory.
  --remove-node-modules  Remove frontend node_modules directory.
  -h, --help             Show this help.

Examples:
  scripts/repo_hygiene.sh
  scripts/repo_hygiene.sh --apply --remove-target
  scripts/repo_hygiene.sh --apply --remove-target --remove-node-modules --prune-empty-dirs
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)
      APPLY=1
      ;;
    --strict-source)
      STRICT_SOURCE=1
      ;;
    --prune-empty-dirs)
      PRUNE_EMPTY_DIRS=1
      ;;
    --remove-target)
      REMOVE_TARGET=1
      ;;
    --remove-node-modules)
      REMOVE_NODE_MODULES=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

echo "[repo-hygiene] root: $ROOT_DIR"

echo "[repo-hygiene] disk usage snapshot:"
du -sh \
  "$ROOT_DIR/.xenobot" \
  "$ROOT_DIR/crates" \
  "$ROOT_DIR/crates/web/frontend" \
  "$ROOT_DIR/docs" \
  "$ROOT_DIR/reports" \
  "$ROOT_DIR/scripts" \
  "$TARGET_DIR" \
  2>/dev/null | sort -h

ds_store_count="$(
  find "$ROOT_DIR" \
    -path "$ROOT_DIR/.git" -prune -o \
    -type f -name '.DS_Store' -print | wc -l | tr -d '[:space:]'
)"
echo "[repo-hygiene] .DS_Store files: $ds_store_count"

empty_dir_count="$(
  find "$ROOT_DIR" \
    -path "$ROOT_DIR/.git" -prune -o \
    -path "$TARGET_DIR" -prune -o \
    -path "$FRONTEND_NODE_MODULES" -prune -o \
    -type d -empty -print | wc -l | tr -d '[:space:]'
)"
echo "[repo-hygiene] empty dirs (excluding .git/target/node_modules): $empty_dir_count"

source_ds_store_count="$(
  find "$ROOT_DIR" \
    -path "$ROOT_DIR/.git" -prune -o \
    -path "$TARGET_DIR" -prune -o \
    -path "$ROOT_DIR/.xenobot" -prune -o \
    -path "$ROOT_DIR/.Xenobot" -prune -o \
    -path "$FRONTEND_NODE_MODULES" -prune -o \
    -type f -name '.DS_Store' -print | wc -l | tr -d '[:space:]'
)"
echo "[repo-hygiene] source tree .DS_Store files (strict scope): $source_ds_store_count"

if [[ $STRICT_SOURCE -eq 1 && "$source_ds_store_count" != "0" ]]; then
  echo "[repo-hygiene] strict-source check failed: remove source-tree .DS_Store files." >&2
  exit 1
fi

if [[ $APPLY -ne 1 ]]; then
  echo "[repo-hygiene] audit complete (no files removed)."
  echo "[repo-hygiene] use --apply to execute cleanup actions."
  exit 0
fi

echo "[repo-hygiene] apply mode enabled."

if [[ "$ds_store_count" != "0" ]]; then
  find "$ROOT_DIR" \
    -path "$ROOT_DIR/.git" -prune -o \
    -type f -name '.DS_Store' -print -delete
  echo "[repo-hygiene] removed .DS_Store files."
else
  echo "[repo-hygiene] no .DS_Store files to remove."
fi

if [[ $REMOVE_TARGET -eq 1 ]]; then
  if [[ -d "$TARGET_DIR" ]]; then
    rm -rf "$TARGET_DIR"
    echo "[repo-hygiene] removed target directory."
  else
    echo "[repo-hygiene] target directory not present."
  fi
fi

if [[ $REMOVE_NODE_MODULES -eq 1 ]]; then
  if [[ -d "$FRONTEND_NODE_MODULES" ]]; then
    rm -rf "$FRONTEND_NODE_MODULES"
    echo "[repo-hygiene] removed frontend node_modules."
  else
    echo "[repo-hygiene] frontend node_modules not present."
  fi
fi

if [[ $PRUNE_EMPTY_DIRS -eq 1 ]]; then
  find "$ROOT_DIR" \
    -path "$ROOT_DIR/.git" -prune -o \
    -path "$TARGET_DIR" -prune -o \
    -path "$FRONTEND_NODE_MODULES" -prune -o \
    -type d -empty -print -delete
  echo "[repo-hygiene] pruned empty directories."
fi

echo "[repo-hygiene] cleanup complete."
