#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

FEATURES="api,analysis"

PLATFORM_PACKAGES=(
  xenobot-wechat
  xenobot-whatsapp
  xenobot-line
  xenobot-qq
  xenobot-telegram
  xenobot-discord
  xenobot-instagram
  xenobot-imessage
  xenobot-messenger
  xenobot-kakaotalk
  xenobot-slack
  xenobot-teams
  xenobot-signal
  xenobot-skype
  xenobot-googlechat
  xenobot-zoom
  xenobot-viber
)

echo "[platform-coverage] checking core platform discovery tests"
cargo test -p xenobot-core --offline

echo "[platform-coverage] checking adapter crates"
for pkg in "${PLATFORM_PACKAGES[@]}"; do
  echo "  - cargo test -p ${pkg} --offline"
  cargo test -p "$pkg" --offline
done

echo "[platform-coverage] checking api + cli compile contract"
cargo check -p xenobot-api -p xenobot-cli --features "$FEATURES" --offline

echo "[platform-coverage] complete"
