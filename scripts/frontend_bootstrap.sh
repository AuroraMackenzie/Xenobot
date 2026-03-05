#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
FRONTEND_DIR="${PROJECT_ROOT}/crates/web/frontend"
REGISTRY_URL="https://registry.npmjs.org/"
PING_URL="https://registry.npmjs.org/-/ping"
OFFLINE_BUNDLE_SCRIPT="${PROJECT_ROOT}/scripts/frontend_offline_bundle.sh"

STRICT_MODE=0
WITH_TYPECHECK=0
EXTREME_MODE=0
OFFLINE_BUNDLE_PATH=""

usage() {
  cat <<'EOF'
Usage:
  scripts/frontend_bootstrap.sh [--strict] [--with-typecheck] [--extreme] [--offline-bundle <path>]

Options:
  --strict          Exit non-zero if network/bootstrap cannot be completed.
  --with-typecheck  Run frontend type-check after dependency install.
  --extreme         Disable all network attempts and use local/offline assets only.
  --offline-bundle  Path to offline bundle created by scripts/frontend_offline_bundle.sh.

Notes:
  - Uses official npm registry only: https://registry.npmjs.org/
  - Optimized for Apple Silicon (arm64) dependency resolution path.
  - If DNS/network is unavailable, default behavior is skip-with-message and exit 0.
  - In extreme mode, bootstrap tries existing node_modules or an offline bundle.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --strict)
      STRICT_MODE=1
      ;;
    --with-typecheck)
      WITH_TYPECHECK=1
      ;;
    --extreme)
      EXTREME_MODE=1
      ;;
    --offline-bundle)
      shift
      [[ $# -gt 0 ]] || {
        echo "error: missing value for --offline-bundle" >&2
        exit 2
      }
      OFFLINE_BUNDLE_PATH="$1"
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

if [[ ! -d "${FRONTEND_DIR}" ]]; then
  echo "error: frontend directory not found: ${FRONTEND_DIR}" >&2
  exit 1
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "error: pnpm is not installed or not in PATH" >&2
  exit 1
fi

SYSTEM_NAME="$(uname -s)"
MACHINE_ARCH="$(uname -m)"
if [[ "${SYSTEM_NAME}" != "Darwin" || "${MACHINE_ARCH}" != "arm64" ]]; then
  echo "[frontend-bootstrap] notice: detected ${SYSTEM_NAME}/${MACHINE_ARCH}; target path is tuned for macOS arm64."
fi

restore_offline_bundle_if_requested() {
  if [[ -z "${OFFLINE_BUNDLE_PATH}" ]]; then
    return 1
  fi
  if [[ ! -x "${OFFLINE_BUNDLE_SCRIPT}" ]]; then
    echo "[frontend-bootstrap] offline bundle script not executable: ${OFFLINE_BUNDLE_SCRIPT}"
    return 1
  fi
  echo "[frontend-bootstrap] restoring offline bundle: ${OFFLINE_BUNDLE_PATH}"
  "${OFFLINE_BUNDLE_SCRIPT}" restore --input "${OFFLINE_BUNDLE_PATH}" --clean-node-modules
  return 0
}

run_typecheck_if_requested() {
  if [[ ${WITH_TYPECHECK} -eq 1 ]]; then
    echo "[frontend-bootstrap] running frontend type-check..."
    pnpm -C "${FRONTEND_DIR}" run type-check:web
  fi
}

if [[ ! -f "${FRONTEND_DIR}/pnpm-lock.yaml" ]]; then
  echo "[frontend-bootstrap] notice: pnpm-lock.yaml not found in ${FRONTEND_DIR}."
  echo "[frontend-bootstrap] notice: online install will use --no-frozen-lockfile until lockfile is generated."
fi

if [[ ${EXTREME_MODE} -eq 1 ]]; then
  echo "[frontend-bootstrap] extreme mode enabled: network access disabled."
  if [[ -d "${FRONTEND_DIR}/node_modules" ]]; then
    echo "[frontend-bootstrap] existing node_modules detected, reuse local dependencies."
    run_typecheck_if_requested
    echo "[frontend-bootstrap] completed (extreme mode, local dependencies)."
    exit 0
  fi

  if restore_offline_bundle_if_requested; then
    run_typecheck_if_requested
    echo "[frontend-bootstrap] completed (extreme mode, offline bundle restored)."
    exit 0
  fi

  echo "[frontend-bootstrap] no local node_modules and no usable offline bundle."
  if [[ ${STRICT_MODE} -eq 1 ]]; then
    exit 1
  fi
  exit 0
fi

echo "[frontend-bootstrap] checking official npm registry reachability..."
if ! curl -fsS --max-time 8 "${PING_URL}" >/dev/null 2>&1; then
  echo "[frontend-bootstrap] network/DNS check failed for ${PING_URL}"
  if restore_offline_bundle_if_requested; then
    run_typecheck_if_requested
    echo "[frontend-bootstrap] completed using offline bundle after network failure."
    exit 0
  fi
  echo "[frontend-bootstrap] skip frontend install. Rust backend development remains available."
  if [[ ${STRICT_MODE} -eq 1 ]]; then
    exit 1
  fi
  exit 0
fi

echo "[frontend-bootstrap] registry reachable, installing dependencies from official npm registry..."
(
  cd "${FRONTEND_DIR}"
  INSTALL_MODE="--no-frozen-lockfile"
  if [[ -f "${FRONTEND_DIR}/pnpm-lock.yaml" ]]; then
    INSTALL_MODE="--frozen-lockfile"
  fi
  npm_config_arch=arm64 \
  npm_config_target_arch=arm64 \
  pnpm install "${INSTALL_MODE}" --registry "${REGISTRY_URL}"
)

run_typecheck_if_requested

echo "[frontend-bootstrap] completed"
