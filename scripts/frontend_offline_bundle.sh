#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
FRONTEND_DIR="${PROJECT_ROOT}/crates/web/frontend"
DEFAULT_BUNDLE_DIR="${PROJECT_ROOT}/.xenobot/offline"
DEFAULT_BUNDLE_PATH="${DEFAULT_BUNDLE_DIR}/frontend-offline-bundle.tar.gz"

usage() {
  cat <<'EOF'
Usage:
  scripts/frontend_offline_bundle.sh create [--output <bundle.tar.gz>]
  scripts/frontend_offline_bundle.sh restore [--input <bundle.tar.gz>] [--clean-node-modules] [--allow-missing-checksum] [--allow-missing-manifest] [--skip-checksum] [--skip-manifest]
  scripts/frontend_offline_bundle.sh info [--input <bundle.tar.gz>]
  scripts/frontend_offline_bundle.sh verify [--input <bundle.tar.gz>] [--allow-missing-checksum] [--allow-missing-manifest] [--skip-checksum] [--skip-manifest]

Notes:
  - Bundle includes frontend node_modules and current pnpm store snapshot.
  - A SHA256 sidecar file is generated as <bundle.tar.gz>.sha256.
  - A metadata sidecar file is generated as <bundle.tar.gz>.manifest.json.
  - Restore verifies manifest and checksum by default.
  - Designed for offline/extreme environments on macOS arm64.
  - Bundle path defaults to .xenobot/offline/frontend-offline-bundle.tar.gz
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

require_pnpm() {
  command -v pnpm >/dev/null 2>&1 || die "pnpm is not installed or not in PATH"
}

resolve_store_path() {
  pnpm store path
}

ensure_frontend_dir() {
  [[ -d "${FRONTEND_DIR}" ]] || die "frontend directory not found: ${FRONTEND_DIR}"
}

checksum_path_for_bundle() {
  local bundle_path="$1"
  printf "%s.sha256" "${bundle_path}"
}

sidecar_manifest_path_for_bundle() {
  local bundle_path="$1"
  printf "%s.manifest.json" "${bundle_path}"
}

compute_sha256() {
  local target_file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${target_file}" | awk '{print $1}'
    return 0
  fi
  if command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "${target_file}" | awk '{print $NF}'
    return 0
  fi
  die "no SHA256 tool found (requires shasum or openssl)"
}

write_bundle_checksum() {
  local bundle_path="$1"
  local checksum_path
  checksum_path="$(checksum_path_for_bundle "${bundle_path}")"
  local digest
  digest="$(compute_sha256 "${bundle_path}")"
  printf "%s  %s\n" "${digest}" "$(basename "${bundle_path}")" > "${checksum_path}"
  echo "[frontend-offline-bundle] checksum written: ${checksum_path}"
}

write_bundle_sidecar_manifest() {
  local bundle_path="$1"
  local internal_manifest_path="$2"
  local sidecar_path
  sidecar_path="$(sidecar_manifest_path_for_bundle "${bundle_path}")"
  local digest
  local bundle_size
  digest="$(compute_sha256 "${bundle_path}")"
  bundle_size="$(wc -c < "${bundle_path}" | tr -d ' ')"
  local created_at_utc
  local system_name
  local arch_name
  local pnpm_version
  local node_version
  local store_path
  created_at_utc="$(read_manifest_value "${internal_manifest_path}" "created_at_utc")"
  system_name="$(read_manifest_value "${internal_manifest_path}" "system")"
  arch_name="$(read_manifest_value "${internal_manifest_path}" "arch")"
  pnpm_version="$(read_manifest_value "${internal_manifest_path}" "pnpm_version")"
  node_version="$(read_manifest_value "${internal_manifest_path}" "node_version")"
  store_path="$(read_manifest_value "${internal_manifest_path}" "store_path")"
  cat > "${sidecar_path}" <<EOF
{
  "bundle_file": "$(basename "${bundle_path}")",
  "bundle_size_bytes": ${bundle_size},
  "sha256": "${digest}",
  "created_at_utc": "${created_at_utc}",
  "system": "${system_name}",
  "arch": "${arch_name}",
  "pnpm_version": "${pnpm_version}",
  "node_version": "${node_version}",
  "store_path": "${store_path}"
}
EOF
  echo "[frontend-offline-bundle] metadata manifest written: ${sidecar_path}"
}

read_expected_checksum() {
  local checksum_path="$1"
  awk 'NR==1 {print $1}' "${checksum_path}"
}

verify_bundle_checksum() {
  local bundle_path="$1"
  local allow_missing="$2"
  local skip_checksum="$3"

  if [[ "${skip_checksum}" -eq 1 ]]; then
    echo "[frontend-offline-bundle] checksum verification skipped by option."
    return 0
  fi

  local checksum_path
  checksum_path="$(checksum_path_for_bundle "${bundle_path}")"
  if [[ ! -f "${checksum_path}" ]]; then
    if [[ "${allow_missing}" -eq 1 ]]; then
      echo "[frontend-offline-bundle] notice: checksum file missing (${checksum_path}), continue by option."
      return 0
    fi
    die "checksum file missing: ${checksum_path} (use --allow-missing-checksum to bypass)"
  fi

  local expected actual
  expected="$(read_expected_checksum "${checksum_path}")"
  [[ -n "${expected}" ]] || die "checksum file is invalid: ${checksum_path}"
  actual="$(compute_sha256 "${bundle_path}")"
  if [[ "${expected}" != "${actual}" ]]; then
    die "checksum mismatch for ${bundle_path}: expected ${expected}, got ${actual}"
  fi
  echo "[frontend-offline-bundle] checksum verified: ${checksum_path}"
}

extract_json_string_value() {
  local json_path="$1"
  local key="$2"
  sed -nE "s/^[[:space:]]*\"${key}\"[[:space:]]*:[[:space:]]*\"([^\"]*)\".*/\\1/p" "${json_path}" | head -n 1
}

extract_json_number_value() {
  local json_path="$1"
  local key="$2"
  sed -nE "s/^[[:space:]]*\"${key}\"[[:space:]]*:[[:space:]]*([0-9]+).*/\\1/p" "${json_path}" | head -n 1
}

verify_bundle_sidecar_manifest() {
  local bundle_path="$1"
  local allow_missing="$2"
  local skip_manifest="$3"
  local sidecar_path
  sidecar_path="$(sidecar_manifest_path_for_bundle "${bundle_path}")"

  if [[ "${skip_manifest}" -eq 1 ]]; then
    echo "[frontend-offline-bundle] metadata manifest verification skipped by option."
    return 0
  fi

  if [[ ! -f "${sidecar_path}" ]]; then
    if [[ "${allow_missing}" -eq 1 ]]; then
      echo "[frontend-offline-bundle] notice: metadata manifest missing (${sidecar_path}), continue by option."
      return 0
    fi
    die "metadata manifest missing: ${sidecar_path} (use --allow-missing-manifest to bypass)"
  fi

  local expected_sha actual_sha expected_size actual_size
  expected_sha="$(extract_json_string_value "${sidecar_path}" "sha256")"
  expected_size="$(extract_json_number_value "${sidecar_path}" "bundle_size_bytes")"
  [[ -n "${expected_sha}" ]] || die "invalid metadata manifest: missing sha256 in ${sidecar_path}"
  [[ -n "${expected_size}" ]] || die "invalid metadata manifest: missing bundle_size_bytes in ${sidecar_path}"

  actual_sha="$(compute_sha256 "${bundle_path}")"
  if [[ "${expected_sha}" != "${actual_sha}" ]]; then
    die "metadata manifest sha256 mismatch for ${bundle_path}: expected ${expected_sha}, got ${actual_sha}"
  fi

  actual_size="$(wc -c < "${bundle_path}" | tr -d ' ')"
  if [[ "${expected_size}" != "${actual_size}" ]]; then
    die "metadata manifest size mismatch for ${bundle_path}: expected ${expected_size}, got ${actual_size}"
  fi

  echo "[frontend-offline-bundle] metadata manifest verified: ${sidecar_path}"
}

verify_manifest_file() {
  local manifest_path="$1"
  [[ -f "${manifest_path}" ]] || die "bundle has no manifest.txt"
  local required_keys=(
    "created_at_utc"
    "system"
    "arch"
    "pnpm_version"
    "node_version"
    "store_path"
    "frontend_dir"
  )
  local key
  for key in "${required_keys[@]}"; do
    grep -q "^${key}=" "${manifest_path}" || die "manifest missing key '${key}': ${manifest_path}"
  done
}

read_manifest_value() {
  local manifest_path="$1"
  local key="$2"
  grep "^${key}=" "${manifest_path}" | head -n 1 | cut -d'=' -f2-
}

bundle_create() {
  local output_path="${DEFAULT_BUNDLE_PATH}"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --output)
        shift
        [[ $# -gt 0 ]] || die "missing value for --output"
        output_path="$1"
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown option for create: $1"
        ;;
    esac
    shift
  done

  require_pnpm
  ensure_frontend_dir

  local store_path
  store_path="$(resolve_store_path)"
  [[ -d "${FRONTEND_DIR}/node_modules" ]] || die "frontend node_modules not found; run bootstrap online first"
  [[ -d "${store_path}" ]] || die "pnpm store path not found: ${store_path}"

  mkdir -p "$(dirname "${output_path}")"
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir}"' EXIT

  mkdir -p "${tmp_dir}/payload/frontend"
  mkdir -p "${tmp_dir}/payload/pnpm-store"

  cp -a "${FRONTEND_DIR}/node_modules" "${tmp_dir}/payload/frontend/node_modules"
  cp -f "${FRONTEND_DIR}/package.json" "${tmp_dir}/payload/frontend/package.json"
  [[ -f "${FRONTEND_DIR}/pnpm-lock.yaml" ]] && cp -f "${FRONTEND_DIR}/pnpm-lock.yaml" "${tmp_dir}/payload/frontend/pnpm-lock.yaml"
  [[ -f "${FRONTEND_DIR}/.npmrc" ]] && cp -f "${FRONTEND_DIR}/.npmrc" "${tmp_dir}/payload/frontend/.npmrc"
  cp -a "${store_path}/." "${tmp_dir}/payload/pnpm-store/"

  cat > "${tmp_dir}/manifest.txt" <<EOF
created_at_utc=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
system=$(uname -s)
arch=$(uname -m)
pnpm_version=$(pnpm --version 2>/dev/null || echo unknown)
node_version=$(node --version 2>/dev/null || echo unknown)
store_path=${store_path}
frontend_dir=${FRONTEND_DIR}
EOF

  tar -czf "${output_path}" -C "${tmp_dir}" payload manifest.txt
  write_bundle_checksum "${output_path}"
  write_bundle_sidecar_manifest "${output_path}" "${tmp_dir}/manifest.txt"
  echo "[frontend-offline-bundle] created: ${output_path}"
}

bundle_restore() {
  local input_path="${DEFAULT_BUNDLE_PATH}"
  local clean_node_modules=0
  local allow_missing_checksum=0
  local allow_missing_manifest=0
  local skip_checksum=0
  local skip_manifest=0

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --input)
        shift
        [[ $# -gt 0 ]] || die "missing value for --input"
        input_path="$1"
        ;;
      --clean-node-modules)
        clean_node_modules=1
        ;;
      --allow-missing-checksum)
        allow_missing_checksum=1
        ;;
      --allow-missing-manifest)
        allow_missing_manifest=1
        ;;
      --skip-checksum)
        skip_checksum=1
        ;;
      --skip-manifest)
        skip_manifest=1
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown option for restore: $1"
        ;;
    esac
    shift
  done

  require_pnpm
  ensure_frontend_dir
  [[ -f "${input_path}" ]] || die "bundle not found: ${input_path}"
  verify_bundle_sidecar_manifest "${input_path}" "${allow_missing_manifest}" "${skip_manifest}"
  verify_bundle_checksum "${input_path}" "${allow_missing_checksum}" "${skip_checksum}"

  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir}"' EXIT
  tar -xzf "${input_path}" -C "${tmp_dir}"

  verify_manifest_file "${tmp_dir}/manifest.txt"
  [[ -d "${tmp_dir}/payload/frontend/node_modules" ]] || die "bundle is invalid: missing payload/frontend/node_modules"
  [[ -d "${tmp_dir}/payload/pnpm-store" ]] || die "bundle is invalid: missing payload/pnpm-store"

  local bundle_arch
  bundle_arch="$(read_manifest_value "${tmp_dir}/manifest.txt" "arch")"
  if [[ -n "${bundle_arch}" && "${bundle_arch}" != "$(uname -m)" ]]; then
    echo "[frontend-offline-bundle] notice: bundle arch '${bundle_arch}' differs from host arch '$(uname -m)'."
  fi

  if [[ ${clean_node_modules} -eq 1 ]]; then
    rm -rf "${FRONTEND_DIR}/node_modules"
  fi

  cp -a "${tmp_dir}/payload/frontend/node_modules" "${FRONTEND_DIR}/node_modules"
  [[ -f "${tmp_dir}/payload/frontend/pnpm-lock.yaml" ]] && cp -f "${tmp_dir}/payload/frontend/pnpm-lock.yaml" "${FRONTEND_DIR}/pnpm-lock.yaml"
  [[ -f "${tmp_dir}/payload/frontend/.npmrc" ]] && cp -f "${tmp_dir}/payload/frontend/.npmrc" "${FRONTEND_DIR}/.npmrc"

  local store_path
  store_path="$(resolve_store_path)"
  mkdir -p "${store_path}"
  cp -a "${tmp_dir}/payload/pnpm-store/." "${store_path}/"

  echo "[frontend-offline-bundle] restored from: ${input_path}"
}

bundle_info() {
  local input_path="${DEFAULT_BUNDLE_PATH}"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --input)
        shift
        [[ $# -gt 0 ]] || die "missing value for --input"
        input_path="$1"
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown option for info: $1"
        ;;
    esac
    shift
  done

  [[ -f "${input_path}" ]] || die "bundle not found: ${input_path}"
  local checksum_path
  checksum_path="$(checksum_path_for_bundle "${input_path}")"
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir}"' EXIT
  tar -xzf "${input_path}" -C "${tmp_dir}"
  verify_manifest_file "${tmp_dir}/manifest.txt"
  echo "[frontend-offline-bundle] bundle: ${input_path}"
  local sidecar_manifest_path
  sidecar_manifest_path="$(sidecar_manifest_path_for_bundle "${input_path}")"
  if [[ -f "${sidecar_manifest_path}" ]]; then
    verify_bundle_sidecar_manifest "${input_path}" 1 0
  else
    echo "[frontend-offline-bundle] notice: metadata manifest missing (${sidecar_manifest_path})"
  fi
  if [[ -f "${checksum_path}" ]]; then
    verify_bundle_checksum "${input_path}" 1 0
  else
    echo "[frontend-offline-bundle] notice: checksum file missing (${checksum_path})"
  fi
  cat "${tmp_dir}/manifest.txt"
  if [[ -f "${sidecar_manifest_path}" ]]; then
    cat "${sidecar_manifest_path}"
  fi
}

bundle_verify() {
  local input_path="${DEFAULT_BUNDLE_PATH}"
  local allow_missing_checksum=0
  local allow_missing_manifest=0
  local skip_checksum=0
  local skip_manifest=0

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --input)
        shift
        [[ $# -gt 0 ]] || die "missing value for --input"
        input_path="$1"
        ;;
      --allow-missing-checksum)
        allow_missing_checksum=1
        ;;
      --allow-missing-manifest)
        allow_missing_manifest=1
        ;;
      --skip-checksum)
        skip_checksum=1
        ;;
      --skip-manifest)
        skip_manifest=1
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown option for verify: $1"
        ;;
    esac
    shift
  done

  [[ -f "${input_path}" ]] || die "bundle not found: ${input_path}"
  verify_bundle_sidecar_manifest "${input_path}" "${allow_missing_manifest}" "${skip_manifest}"
  verify_bundle_checksum "${input_path}" "${allow_missing_checksum}" "${skip_checksum}"
  local tmp_dir
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir}"' EXIT
  tar -xzf "${input_path}" -C "${tmp_dir}"
  verify_manifest_file "${tmp_dir}/manifest.txt"
  [[ -d "${tmp_dir}/payload/frontend/node_modules" ]] || die "bundle is invalid: missing payload/frontend/node_modules"
  [[ -d "${tmp_dir}/payload/pnpm-store" ]] || die "bundle is invalid: missing payload/pnpm-store"
  echo "[frontend-offline-bundle] verify: OK (${input_path})"
}

main() {
  [[ $# -gt 0 ]] || {
    usage
    exit 2
  }

  case "$1" in
    create)
      shift
      bundle_create "$@"
      ;;
    restore)
      shift
      bundle_restore "$@"
      ;;
    info)
      shift
      bundle_info "$@"
      ;;
    verify)
      shift
      bundle_verify "$@"
      ;;
    -h|--help)
      usage
      ;;
    *)
      die "unknown subcommand: $1"
      ;;
  esac
}

main "$@"
