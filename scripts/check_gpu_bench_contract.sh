#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SIZE="${SIZE:-64}"
ITERS="${ITERS:-2}"

if [[ $# -gt 0 ]]; then
  SIZE="$1"
fi
if [[ $# -gt 1 ]]; then
  ITERS="$2"
fi

if ! [[ "$SIZE" =~ ^[0-9]+$ ]] || ! [[ "$ITERS" =~ ^[0-9]+$ ]]; then
  echo "error: size and iterations must be unsigned integers" >&2
  exit 2
fi

json_output="$(
  cd "$ROOT"
  cargo run -q -p xenobot-gpu --bin xenobot-gpu-bench --offline -- \
    --size "$SIZE" --iters "$ITERS" --format json
)"

JSON_OUTPUT="$json_output" python3 - "$SIZE" "$ITERS" <<'PY'
import json
import math
import os
import sys

expected_size = int(sys.argv[1])
expected_iters = int(sys.argv[2])
payload = json.loads(os.environ["JSON_OUTPUT"])

required = [
    "size",
    "iterations",
    "cpuAvgMs",
    "cpuGflops",
    "gpuAvailable",
    "gpuDevice",
    "gpuAvgMs",
    "gpuGflops",
    "maxAbsDiff",
    "error",
]
missing = [k for k in required if k not in payload]
if missing:
    print(f"error: missing keys: {missing}", file=sys.stderr)
    sys.exit(1)

if payload["size"] != expected_size:
    print(
        f"error: size mismatch, expected {expected_size}, got {payload['size']}",
        file=sys.stderr,
    )
    sys.exit(1)

if payload["iterations"] != expected_iters:
    print(
        f"error: iterations mismatch, expected {expected_iters}, got {payload['iterations']}",
        file=sys.stderr,
    )
    sys.exit(1)

cpu_avg_ms = payload["cpuAvgMs"]
cpu_gflops = payload["cpuGflops"]
if not isinstance(cpu_avg_ms, (int, float)) or cpu_avg_ms <= 0:
    print(f"error: invalid cpuAvgMs: {cpu_avg_ms}", file=sys.stderr)
    sys.exit(1)

if not isinstance(cpu_gflops, (int, float)) or not math.isfinite(cpu_gflops) or cpu_gflops <= 0:
    print(f"error: invalid cpuGflops: {cpu_gflops}", file=sys.stderr)
    sys.exit(1)

gpu_available = payload["gpuAvailable"]
if not isinstance(gpu_available, bool):
    print(f"error: gpuAvailable must be boolean, got {type(gpu_available)}", file=sys.stderr)
    sys.exit(1)

if gpu_available:
    if payload["gpuAvgMs"] is None or payload["gpuGflops"] is None:
        print("error: gpuAvailable=true requires gpuAvgMs/gpuGflops", file=sys.stderr)
        sys.exit(1)
else:
    if payload["error"] is None:
        print("error: gpuAvailable=false should include an error description", file=sys.stderr)
        sys.exit(1)

print("gpu benchmark contract: ok")
PY
