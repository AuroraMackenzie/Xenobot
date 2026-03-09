#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

MESSAGES=20000
DB_PATH="/tmp/xenobot-perf.db"
GPU_SIZE=256
GPU_ITERS=8
SKIP_GPU=0
OUTPUT_PATH=""

usage() {
  cat <<'EOF'
Usage:
  perf_baseline.sh [options]

Options:
  --messages <n>      Number of synthetic Telegram-style JSONL messages (default: 20000)
  --db-path <path>    Target SQLite path (default: /tmp/xenobot-perf.db)
  --gpu-size <n>      Square matrix size for gpu benchmark (default: 256)
  --gpu-iters <n>     Iterations for gpu benchmark (default: 8)
  --skip-gpu          Skip GPU benchmark stage
  --output <path>     Output report JSON path (default: reports/perf/perf_baseline_<utc>.json)
  -h, --help          Show help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
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
    --gpu-size)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --gpu-size" >&2
        exit 2
      }
      GPU_SIZE="$1"
      ;;
    --gpu-iters)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --gpu-iters" >&2
        exit 2
      }
      GPU_ITERS="$1"
      ;;
    --skip-gpu)
      SKIP_GPU=1
      ;;
    --output)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --output" >&2
        exit 2
      }
      OUTPUT_PATH="$1"
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

if ! [[ "$MESSAGES" =~ ^[0-9]+$ ]] || [[ "$MESSAGES" -lt 100 ]]; then
  echo "error: --messages must be an integer >= 100" >&2
  exit 2
fi
if ! [[ "$GPU_SIZE" =~ ^[0-9]+$ ]] || [[ "$GPU_SIZE" -lt 16 ]]; then
  echo "error: --gpu-size must be an integer >= 16" >&2
  exit 2
fi
if ! [[ "$GPU_ITERS" =~ ^[0-9]+$ ]] || [[ "$GPU_ITERS" -lt 1 ]]; then
  echo "error: --gpu-iters must be an integer >= 1" >&2
  exit 2
fi

TIMESTAMP_UTC="$(date -u +"%Y%m%dT%H%M%SZ")"
if [[ -z "$OUTPUT_PATH" ]]; then
  OUTPUT_PATH="$ROOT_DIR/reports/perf/perf_baseline_${TIMESTAMP_UTC}.json"
fi
mkdir -p "$(dirname "$OUTPUT_PATH")"

REPORT_BASE="$(basename "$OUTPUT_PATH" .json)"
LOG_DIR="$(dirname "$OUTPUT_PATH")/${REPORT_BASE}_logs"
mkdir -p "$LOG_DIR"

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/xenobot-perf-XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

INPUT_FILE="$WORK_DIR/perf_telegram.jsonl"
echo "[perf] generating synthetic dataset: $INPUT_FILE (messages=$MESSAGES)"
awk -v n="$MESSAGES" 'BEGIN {
  for (i = 1; i <= n; i++) {
    sender = i % 32;
    ts = 1710000000 + i;
    printf("{\"sender_id\":\"tg_u%d\",\"sender_name\":\"User %d\",\"timestamp\":%d,\"msg_type\":0,\"content\":\"message-%d\"}\n", sender, sender, ts, i % 200);
  }
}' > "$INPUT_FILE"

MERGE_MESSAGES=$((MESSAGES / 4))
if [[ "$MERGE_MESSAGES" -lt 200 ]]; then
  MERGE_MESSAGES=200
fi
MERGE_INPUT_DIR="$WORK_DIR/perf_merge_inputs"
mkdir -p "$MERGE_INPUT_DIR"
MERGE_FILE_A="$MERGE_INPUT_DIR/merge_a.jsonl"
MERGE_FILE_B="$MERGE_INPUT_DIR/merge_b.jsonl"
MERGE_A_COUNT=$((MERGE_MESSAGES / 2))
MERGE_B_COUNT=$((MERGE_MESSAGES - MERGE_A_COUNT))
echo "[perf] generating merged-import dataset: $MERGE_INPUT_DIR (messages=$MERGE_MESSAGES, files=2)"
awk -v n="$MERGE_A_COUNT" 'BEGIN {
  for (i = 1; i <= n; i++) {
    sender = i % 24;
    ts = 1720000000 + i;
    printf("{\"sender_id\":\"tg_merge_u%d\",\"sender_name\":\"MergeUser %d\",\"timestamp\":%d,\"msg_type\":0,\"content\":\"merge-a-%d\"}\n", sender, sender, ts, i % 150);
  }
}' > "$MERGE_FILE_A"
awk -v n="$MERGE_B_COUNT" -v offset="$MERGE_A_COUNT" 'BEGIN {
  for (i = 1; i <= n; i++) {
    sender = (offset + i) % 24;
    ts = 1720000000 + offset + i;
    printf("{\"sender_id\":\"tg_merge_u%d\",\"sender_name\":\"MergeUser %d\",\"timestamp\":%d,\"msg_type\":0,\"content\":\"merge-b-%d\"}\n", sender, sender, ts, (offset + i) % 150);
  }
}' > "$MERGE_FILE_B"

echo "[perf] warmup build (excluded from timing)"
cargo build --manifest-path "$ROOT_DIR/Cargo.toml" -q -p xenobot-cli --features "api,analysis"
if [[ "$SKIP_GPU" -eq 0 ]]; then
  cargo build --manifest-path "$ROOT_DIR/Cargo.toml" -q -p xenobot-gpu --bin xenobot-gpu-bench
fi

if [[ -f "$DB_PATH" ]]; then
  rm -f "$DB_PATH"
fi

now_ms() {
  python3 - <<'PY'
import time
print(int(time.time() * 1000))
PY
}

run_step() {
  local step="$1"
  shift
  local log_file="$LOG_DIR/${step}.log"
  local start_ms
  local end_ms
  local elapsed_ms
  local status

  start_ms="$(now_ms)"
  set +e
  "$@" >"$log_file" 2>&1
  status=$?
  set -e
  end_ms="$(now_ms)"
  elapsed_ms=$((end_ms - start_ms))

  echo "$status" > "$WORK_DIR/${step}.status"
  echo "$elapsed_ms" > "$WORK_DIR/${step}.duration_ms"

  if [[ "$status" -ne 0 ]]; then
    echo "[perf] step failed: ${step} (status=${status}, log=${log_file})" >&2
    return "$status"
  fi
  echo "[perf] step ok: ${step} (${elapsed_ms} ms)"
}

echo "[perf] running baseline steps"
run_step "db_create" \
  cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -q -p xenobot-cli --features "api,analysis" -- \
  db create "$DB_PATH"

run_step "import_incremental" \
  cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -q -p xenobot-cli --features "api,analysis" -- \
  import "$INPUT_FILE" telegram --db-path "$DB_PATH" --write-db --incremental --session-name "Perf Telegram"

run_step "import_merge_batch" \
  cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -q -p xenobot-cli --features "api,analysis" -- \
  import "$MERGE_INPUT_DIR" telegram --db-path "$DB_PATH" --write-db --merge --session-name "Perf Merge"

run_step "query_search_json" \
  cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -q -p xenobot-cli --features "api,analysis" -- \
  query --db-path "$DB_PATH" search "message-42" -l 200 -f json

GPU_JSON_PATH=""
if [[ "$SKIP_GPU" -eq 0 ]]; then
  GPU_JSON_PATH="$WORK_DIR/gpu_report.json"
  echo "[perf] running gpu benchmark stage"
  if ! run_step "gpu_bench" \
    cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -q -p xenobot-gpu --bin xenobot-gpu-bench --offline -- \
    --size "$GPU_SIZE" --iters "$GPU_ITERS" --format json; then
    echo "[perf] gpu benchmark command failed (continuing with error note in report)" >&2
  fi
  if [[ -f "$LOG_DIR/gpu_bench.log" ]]; then
    tail -n 120 "$LOG_DIR/gpu_bench.log" | awk '
      BEGIN{capture=0}
      /^\{/ {capture=1}
      { if (capture) print $0 }
    ' > "$GPU_JSON_PATH" || true
  fi
fi

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"

python3 - "$OUTPUT_PATH" "$TIMESTAMP_UTC" "$HOST_OS" "$HOST_ARCH" "$MESSAGES" "$DB_PATH" "$INPUT_FILE" "$MERGE_INPUT_DIR" "$MERGE_MESSAGES" "$LOG_DIR" "$GPU_JSON_PATH" <<'PY'
import json
import os
import sys

(
    output_path,
    timestamp_utc,
    host_os,
    host_arch,
    messages,
    db_path,
    input_file,
    merge_input_dir,
    merge_messages,
    log_dir,
    gpu_json_path,
) = sys.argv[1:]

def read_text(path, default=""):
    try:
        with open(path, "r", encoding="utf-8") as f:
            return f.read().strip()
    except Exception:
        return default

def read_int(path, default=None):
    text = read_text(path, "")
    if not text:
        return default
    try:
        return int(text)
    except Exception:
        return default

steps = []
for name in ["db_create", "import_incremental", "import_merge_batch", "query_search_json", "gpu_bench"]:
    status = read_int(os.path.join(os.path.dirname(input_file), f"{name}.status"), None)
    duration = read_int(os.path.join(os.path.dirname(input_file), f"{name}.duration_ms"), None)
    if status is None and duration is None:
        continue
    steps.append(
        {
            "name": name,
            "status": status,
            "durationMs": duration,
            "logPath": os.path.join(log_dir, f"{name}.log"),
        }
    )

step_index = {step["name"]: step for step in steps}
all_steps_ok = all(step.get("status") == 0 for step in steps)
total_duration_ms = sum(
    step.get("durationMs", 0) for step in steps if isinstance(step.get("durationMs"), int)
)

gpu_report = None
if gpu_json_path and os.path.exists(gpu_json_path):
    raw = read_text(gpu_json_path, "")
    if raw:
        try:
            gpu_report = json.loads(raw)
        except Exception:
            gpu_report = {"raw": raw, "parseError": True}

report = {
    "timestampUtc": timestamp_utc,
    "host": {"os": host_os, "arch": host_arch},
    "scenario": {
        "messages": int(messages),
        "dbPath": db_path,
        "inputFile": input_file,
        "mergeInputDir": merge_input_dir,
        "mergeMessages": int(merge_messages),
    },
    "steps": steps,
    "summary": {
        "allStepsOk": all_steps_ok,
        "totalDurationMs": total_duration_ms,
        "stepCount": len(steps),
        "durationsMs": {
            "dbCreate": step_index.get("db_create", {}).get("durationMs"),
            "importIncremental": step_index.get("import_incremental", {}).get("durationMs"),
            "importMergeBatch": step_index.get("import_merge_batch", {}).get("durationMs"),
            "querySearchJson": step_index.get("query_search_json", {}).get("durationMs"),
            "gpuBench": step_index.get("gpu_bench", {}).get("durationMs"),
        },
    },
    "gpuBenchmark": gpu_report,
}

os.makedirs(os.path.dirname(output_path), exist_ok=True)
with open(output_path, "w", encoding="utf-8") as f:
    json.dump(report, f, ensure_ascii=False, indent=2)
PY

echo "[perf] baseline report written: $OUTPUT_PATH"
echo "[perf] logs directory: $LOG_DIR"
