#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

INPUT_PATH=""
MAX_DB_CREATE_MS="${MAX_DB_CREATE_MS:-30000}"
MAX_IMPORT_MS="${MAX_IMPORT_MS:-120000}"
MAX_MERGE_IMPORT_MS="${MAX_MERGE_IMPORT_MS:-120000}"
MAX_QUERY_MS="${MAX_QUERY_MS:-15000}"
MAX_TOTAL_MS="${MAX_TOTAL_MS:-180000}"
MAX_GPU_MS="${MAX_GPU_MS:-0}"
REQUIRE_GPU=0

usage() {
  cat <<'EOF'
Usage:
  check_perf_baseline_contract.sh [options]

Options:
  --input <path>              Baseline report path (default: latest reports/perf/perf_baseline_*.json)
  --max-db-create-ms <n>      Gate for db_create duration (default: 30000)
  --max-import-ms <n>         Gate for import_incremental duration (default: 120000)
  --max-merge-import-ms <n>   Gate for import_merge_batch duration (default: 120000)
  --max-query-ms <n>          Gate for query_search_json duration (default: 15000)
  --max-total-ms <n>          Gate for total baseline duration (default: 180000)
  --max-gpu-ms <n>            Optional GPU duration gate when GPU benchmark data exists (default: disabled)
  --require-gpu               Require gpuBenchmark.gpuAvailable=true
  -h, --help                  Show help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --input)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --input" >&2
        exit 2
      }
      INPUT_PATH="$1"
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

for value in "$MAX_DB_CREATE_MS" "$MAX_IMPORT_MS" "$MAX_MERGE_IMPORT_MS" "$MAX_QUERY_MS" "$MAX_TOTAL_MS" "$MAX_GPU_MS"; do
  if ! [[ "$value" =~ ^[0-9]+$ ]]; then
    echo "error: threshold values must be unsigned integers" >&2
    exit 2
  fi
done

if [[ -z "$INPUT_PATH" ]]; then
  INPUT_PATH="$(ls -1t "$ROOT"/reports/perf/perf_baseline_*.json 2>/dev/null | head -n 1 || true)"
fi

if [[ -z "$INPUT_PATH" ]]; then
  echo "error: no perf baseline report found; run: scripts/xb perf baseline" >&2
  exit 1
fi

if [[ ! -f "$INPUT_PATH" ]]; then
  echo "error: report not found: $INPUT_PATH" >&2
  exit 1
fi

python3 - "$INPUT_PATH" "$MAX_DB_CREATE_MS" "$MAX_IMPORT_MS" "$MAX_MERGE_IMPORT_MS" "$MAX_QUERY_MS" "$MAX_TOTAL_MS" "$MAX_GPU_MS" "$REQUIRE_GPU" <<'PY'
import json
import os
import sys

(
    input_path,
    max_db_create_ms,
    max_import_ms,
    max_merge_import_ms,
    max_query_ms,
    max_total_ms,
    max_gpu_ms,
    require_gpu,
) = sys.argv[1:]

max_db_create_ms = int(max_db_create_ms)
max_import_ms = int(max_import_ms)
max_merge_import_ms = int(max_merge_import_ms)
max_query_ms = int(max_query_ms)
max_total_ms = int(max_total_ms)
max_gpu_ms = int(max_gpu_ms)
require_gpu = bool(int(require_gpu))

with open(input_path, "r", encoding="utf-8") as fh:
    payload = json.load(fh)

def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    sys.exit(1)

for key in ("timestampUtc", "host", "scenario", "steps"):
    if key not in payload:
        fail(f"missing top-level key '{key}'")

if not isinstance(payload["steps"], list):
    fail("'steps' must be an array")

step_map = {}
for step in payload["steps"]:
    if not isinstance(step, dict):
        fail("each step entry must be an object")
    name = step.get("name")
    if not isinstance(name, str) or not name:
        fail("step.name must be non-empty string")
    status = step.get("status")
    if not isinstance(status, int):
        fail(f"step '{name}' status must be int")
    duration = step.get("durationMs")
    if not isinstance(duration, int) or duration < 0:
        fail(f"step '{name}' durationMs must be non-negative int")
    step_map[name] = step

required_steps = ("db_create", "import_incremental", "import_merge_batch", "query_search_json")
for name in required_steps:
    if name not in step_map:
        fail(f"required step missing: {name}")
    if step_map[name]["status"] != 0:
        fail(f"required step failed: {name} (status={step_map[name]['status']})")

def assert_gate(step_name: str, ceiling_ms: int, label: str) -> None:
    duration = int(step_map[step_name]["durationMs"])
    if duration > ceiling_ms:
        fail(f"{label} gate exceeded: {duration}ms > {ceiling_ms}ms")

assert_gate("db_create", max_db_create_ms, "db_create")
assert_gate("import_incremental", max_import_ms, "import_incremental")
assert_gate("import_merge_batch", max_merge_import_ms, "import_merge_batch")
assert_gate("query_search_json", max_query_ms, "query_search_json")

summary = payload.get("summary")
if isinstance(summary, dict) and isinstance(summary.get("totalDurationMs"), int):
    total_duration = int(summary["totalDurationMs"])
else:
    total_duration = sum(int(step["durationMs"]) for step in payload["steps"])

if total_duration > max_total_ms:
    fail(f"totalDuration gate exceeded: {total_duration}ms > {max_total_ms}ms")

gpu_report = payload.get("gpuBenchmark")
gpu_available = None
gpu_avg_ms = None
if isinstance(gpu_report, dict):
    gpu_available = gpu_report.get("gpuAvailable")
    gpu_avg_ms = gpu_report.get("gpuAvgMs")

if require_gpu:
    if gpu_available is not True:
        fail("GPU is required but gpuBenchmark.gpuAvailable is not true")
    if not isinstance(gpu_avg_ms, (int, float)) or gpu_avg_ms <= 0:
        fail("GPU is required but gpuBenchmark.gpuAvgMs is missing/invalid")

if max_gpu_ms > 0 and isinstance(gpu_avg_ms, (int, float)):
    if gpu_avg_ms > max_gpu_ms:
        fail(f"gpuAvgMs gate exceeded: {gpu_avg_ms}ms > {max_gpu_ms}ms")

print(f"perf baseline report: {os.path.abspath(input_path)}")
print(
    "durations ms: "
    f"db_create={step_map['db_create']['durationMs']}, "
    f"import_incremental={step_map['import_incremental']['durationMs']}, "
    f"import_merge_batch={step_map['import_merge_batch']['durationMs']}, "
    f"query_search_json={step_map['query_search_json']['durationMs']}, "
    f"total={total_duration}"
)
if isinstance(gpu_avg_ms, (int, float)):
    print(f"gpu avg ms: {gpu_avg_ms} (available={gpu_available})")
else:
    print(f"gpu avg ms: n/a (available={gpu_available})")
print("perf baseline contract: ok")
PY
