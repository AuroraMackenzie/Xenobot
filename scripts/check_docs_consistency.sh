#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

README="$ROOT_DIR/README.md"
API_DOC="$ROOT_DIR/docs/API.md"
USER_GUIDE="$ROOT_DIR/docs/USER_GUIDE.md"
RUNBOOK="$ROOT_DIR/docs/OPERATIONS_RUNBOOK.md"
QUALITY_GATE_DOC="$ROOT_DIR/docs/QUALITY_GATE.md"

required_files=("$README" "$API_DOC" "$USER_GUIDE" "$RUNBOOK" "$QUALITY_GATE_DOC")
for file in "${required_files[@]}"; do
  [[ -f "$file" ]] || {
    echo "[docs-check] missing required file: $file" >&2
    exit 1
  }
done

require_pattern() {
  local file="$1"
  local pattern="$2"
  local label="$3"
  if ! rg -q "$pattern" "$file"; then
    echo "[docs-check] missing pattern ($label) in $file" >&2
    exit 1
  fi
}

forbid_pattern() {
  local file="$1"
  local pattern="$2"
  local label="$3"
  if rg -q "$pattern" "$file"; then
    echo "[docs-check] forbidden pattern ($label) found in $file" >&2
    exit 1
  fi
}

require_pattern "$README" "docs/OPERATIONS_RUNBOOK.md" "runbook reference in README"
require_pattern "$USER_GUIDE" "docs/OPERATIONS_RUNBOOK.md" "runbook reference in USER_GUIDE"
require_pattern "$QUALITY_GATE_DOC" "docs/OPERATIONS_RUNBOOK.md" "runbook reference in QUALITY_GATE"

require_pattern "$README" "/chat/sessions/:id/generate-sql" "README smoke route prefix"
require_pattern "$USER_GUIDE" "/chat/sessions/:session_id/generate-sql" "USER_GUIDE smoke route prefix"
require_pattern "$API_DOC" "mounted under .*/chat" "API mount note"

forbid_pattern "$README" "POST /sessions/:session_id/generate-sql" "legacy non-prefixed SQL route in README"
forbid_pattern "$USER_GUIDE" "POST /sessions/:session_id/generate-sql" "legacy non-prefixed SQL route in USER_GUIDE"

echo "[docs-check] consistency checks passed"
