#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
FRONTEND_DIR="${PROJECT_ROOT}/crates/web/frontend"
BOOTSTRAP_SCRIPT="${PROJECT_ROOT}/scripts/frontend_bootstrap.sh"
BUNDLE_SCRIPT="${PROJECT_ROOT}/scripts/frontend_offline_bundle.sh"
DEFAULT_BUNDLE_PATH="${PROJECT_ROOT}/.xenobot/offline/frontend-offline-bundle.tar.gz"
PING_URL="https://registry.npmjs.org/-/ping"
REGISTRY_URL="https://registry.npmjs.org/"

REFRESH_LOCKFILE=0
SKIP_TYPECHECK=0
BUNDLE_OUTPUT="${DEFAULT_BUNDLE_PATH}"

usage() {
  cat <<'EOF'
Usage:
  scripts/frontend_deps_update.sh [--refresh-lockfile] [--skip-typecheck] [--bundle-output <path>]

Options:
  --refresh-lockfile  Run install in non-frozen mode to refresh lockfile and dependency graph.
  --skip-typecheck    Skip frontend type-check after install.
  --bundle-output     Output path for generated offline bundle tarball.

Notes:
  - Uses official npm registry only: https://registry.npmjs.org/
  - Designed for online maintenance on macOS arm64.
  - Generates/updates offline bundle for extreme offline environments.
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --refresh-lockfile)
      REFRESH_LOCKFILE=1
      ;;
    --skip-typecheck)
      SKIP_TYPECHECK=1
      ;;
    --bundle-output)
      shift
      [[ $# -gt 0 ]] || die "missing value for --bundle-output"
      BUNDLE_OUTPUT="$1"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
  shift
done

command -v pnpm >/dev/null 2>&1 || die "pnpm is not installed or not in PATH"
command -v curl >/dev/null 2>&1 || die "curl is required"
[[ -x "${BOOTSTRAP_SCRIPT}" ]] || die "bootstrap script missing or not executable: ${BOOTSTRAP_SCRIPT}"
[[ -x "${BUNDLE_SCRIPT}" ]] || die "bundle script missing or not executable: ${BUNDLE_SCRIPT}"
[[ -d "${FRONTEND_DIR}" ]] || die "frontend directory not found: ${FRONTEND_DIR}"

echo "[frontend-deps-update] checking npm registry connectivity..."
curl -fsS --max-time 8 "${PING_URL}" >/dev/null 2>&1 || die "official npm registry unreachable: ${PING_URL}"

echo "[frontend-deps-update] installing frontend dependencies from official npm registry..."
INSTALL_MODE="--frozen-lockfile"
if [[ ! -f "${FRONTEND_DIR}/pnpm-lock.yaml" || ${REFRESH_LOCKFILE} -eq 1 ]]; then
  INSTALL_MODE="--no-frozen-lockfile"
fi

(
  cd "${FRONTEND_DIR}"
  npm_config_arch=arm64 \
  npm_config_target_arch=arm64 \
  pnpm install "${INSTALL_MODE}" --registry "${REGISTRY_URL}"
)

if [[ ${SKIP_TYPECHECK} -eq 0 ]]; then
  echo "[frontend-deps-update] running frontend type-check..."
  pnpm -C "${FRONTEND_DIR}" run type-check:web
fi

echo "[frontend-deps-update] creating offline bundle..."
"${BUNDLE_SCRIPT}" create --output "${BUNDLE_OUTPUT}"

echo "[frontend-deps-update] completed"
echo "[frontend-deps-update] next recommended commands:"
echo "  git add ${FRONTEND_DIR}/pnpm-lock.yaml ${BUNDLE_OUTPUT} ${BUNDLE_OUTPUT}.sha256 ${BUNDLE_OUTPUT}.manifest.json"
echo "  git status --short"
