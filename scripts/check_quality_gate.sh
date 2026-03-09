#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

SKIP_PLATFORM=0
SKIP_PERF=0
SKIP_SMOKE=0
SKIP_MCP=0
MESSAGES=800
DB_PATH="/tmp/xenobot-quality-gate.db"
PERF_OUTPUT="/tmp/xenobot-quality-gate-perf.json"
SMOKE_DB_PATH="/tmp/xenobot-quality-gate-smoke.db"
MAX_DB_CREATE_MS=30000
MAX_IMPORT_MS=120000
MAX_MERGE_IMPORT_MS=120000
MAX_QUERY_MS=15000
MAX_TOTAL_MS=180000
MAX_GPU_MS=0
REQUIRE_GPU=0

usage() {
  cat <<'EOF'
Usage:
  check_quality_gate.sh [options]

Options:
  --skip-platform             Skip platform coverage suite
  --skip-perf                 Skip performance baseline + perf gate
  --skip-smoke                Skip in-process API smoke contract check
  --skip-mcp                  Skip strict MCP suite
  --messages <n>              Synthetic message count for perf baseline (default: 800)
  --db-path <path>            SQLite path used in perf baseline (default: /tmp/xenobot-quality-gate.db)
  --perf-output <path>        Perf report output path (default: /tmp/xenobot-quality-gate-perf.json)
  --smoke-db-path <path>      SQLite path used in API smoke check (default: /tmp/xenobot-quality-gate-smoke.db)
  --max-db-create-ms <n>      Perf gate threshold for db_create
  --max-import-ms <n>         Perf gate threshold for import_incremental
  --max-merge-import-ms <n>   Perf gate threshold for import_merge_batch
  --max-query-ms <n>          Perf gate threshold for query_search_json
  --max-total-ms <n>          Perf gate threshold for total duration
  --max-gpu-ms <n>            Optional GPU avg-ms threshold
  --require-gpu               Require gpuBenchmark.gpuAvailable=true
  -h, --help                  Show help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-platform)
      SKIP_PLATFORM=1
      ;;
    --skip-perf)
      SKIP_PERF=1
      ;;
    --skip-smoke)
      SKIP_SMOKE=1
      ;;
    --skip-mcp)
      SKIP_MCP=1
      ;;
    --messages)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --messages" >&2
        exit 2
      }
      MESSAGES="$1"
      ;;
    --db-path)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --db-path" >&2
        exit 2
      }
      DB_PATH="$1"
      ;;
    --perf-output)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --perf-output" >&2
        exit 2
      }
      PERF_OUTPUT="$1"
      ;;
    --smoke-db-path)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --smoke-db-path" >&2
        exit 2
      }
      SMOKE_DB_PATH="$1"
      ;;
    --max-db-create-ms)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --max-db-create-ms" >&2
        exit 2
      }
      MAX_DB_CREATE_MS="$1"
      ;;
    --max-import-ms)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --max-import-ms" >&2
        exit 2
      }
      MAX_IMPORT_MS="$1"
      ;;
    --max-merge-import-ms)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --max-merge-import-ms" >&2
        exit 2
      }
      MAX_MERGE_IMPORT_MS="$1"
      ;;
    --max-query-ms)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --max-query-ms" >&2
        exit 2
      }
      MAX_QUERY_MS="$1"
      ;;
    --max-total-ms)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --max-total-ms" >&2
        exit 2
      }
      MAX_TOTAL_MS="$1"
      ;;
    --max-gpu-ms)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --max-gpu-ms" >&2
        exit 2
      }
      MAX_GPU_MS="$1"
      ;;
    --require-gpu)
      REQUIRE_GPU=1
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

for value in "$MESSAGES" "$MAX_DB_CREATE_MS" "$MAX_IMPORT_MS" "$MAX_MERGE_IMPORT_MS" "$MAX_QUERY_MS" "$MAX_TOTAL_MS" "$MAX_GPU_MS"; do
  if ! [[ "$value" =~ ^[0-9]+$ ]]; then
    echo "error: numeric option expects unsigned integer" >&2
    exit 2
  fi
done

if [[ "$MESSAGES" -lt 100 ]]; then
  echo "error: --messages must be >= 100" >&2
  exit 2
fi

run_step() {
  local title="$1"
  shift
  echo "[quality-gate] $title"
  "$@"
}

cd "$ROOT_DIR"

run_step "strict workspace check" \
  env RUSTFLAGS='-D warnings' cargo check --workspace --offline

run_step "docs consistency check" \
  "$ROOT_DIR/scripts/check_docs_consistency.sh"

run_step "source hygiene pre-clean" \
  "$ROOT_DIR/scripts/repo_hygiene.sh" --apply

run_step "source hygiene check" \
  "$ROOT_DIR/scripts/repo_hygiene.sh" --strict-source

run_step "strict cli suite" \
  env RUSTFLAGS='-D warnings' cargo test -p xenobot-cli --features "api,analysis" --offline

run_step "strict api suite" \
  env RUSTFLAGS='-D warnings' cargo test -p xenobot-api --offline

if [[ "$SKIP_MCP" -eq 0 ]]; then
  run_step "strict mcp suite" \
    env RUSTFLAGS='-D warnings' cargo test -p xenobot-mcp --offline
else
  echo "[quality-gate] mcp suite skipped by flag"
fi

if [[ "$SKIP_SMOKE" -eq 0 ]]; then
  run_step "api smoke contract check" \
    cargo run -p xenobot-cli --features "api,analysis" -- \
      api smoke --db-path "$SMOKE_DB_PATH"
else
  echo "[quality-gate] api smoke check skipped by flag"
fi

if [[ "$SKIP_PLATFORM" -eq 0 ]]; then
  run_step "platform coverage suite" \
    env RUSTFLAGS='-D warnings' "$ROOT_DIR/scripts/check_platform_coverage.sh"
else
  echo "[quality-gate] platform coverage skipped by flag"
fi

if [[ "$SKIP_PERF" -eq 0 ]]; then
  run_step "performance baseline generation" \
    "$ROOT_DIR/scripts/xb" perf baseline \
      --messages "$MESSAGES" \
      --db-path "$DB_PATH" \
      --skip-gpu \
      --output "$PERF_OUTPUT"

  perf_check_args=(
    --input "$PERF_OUTPUT"
    --max-db-create-ms "$MAX_DB_CREATE_MS"
    --max-import-ms "$MAX_IMPORT_MS"
    --max-merge-import-ms "$MAX_MERGE_IMPORT_MS"
    --max-query-ms "$MAX_QUERY_MS"
    --max-total-ms "$MAX_TOTAL_MS"
  )
  if [[ "$MAX_GPU_MS" -gt 0 ]]; then
    perf_check_args+=(--max-gpu-ms "$MAX_GPU_MS")
  fi
  if [[ "$REQUIRE_GPU" -eq 1 ]]; then
    perf_check_args+=(--require-gpu)
  fi

  run_step "performance contract check" \
    "$ROOT_DIR/scripts/xb" perf check "${perf_check_args[@]}"
else
  echo "[quality-gate] performance gate skipped by flag"
fi

echo "[quality-gate] all checks passed"
