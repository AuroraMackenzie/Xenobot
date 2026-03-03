#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PATTERN='//.*\p{Han}|/\*.*\p{Han}|^\s*\*.*\p{Han}|^\s*<!--.*\p{Han}'

if rg -n --pcre2 "$PATTERN" crates --glob '*.{rs,ts,tsx,js,jsx,vue,css,scss,html}' >/tmp/xenobot_non_english_comments.txt; then
  echo "Non-English comments detected in source files:"
  cat /tmp/xenobot_non_english_comments.txt
  exit 1
fi

echo "Comment language check passed: no Han-character comments found in source code."
