#!/bin/sh
# reapply.sh — restore the /etc bits after a reMarkable OS update.
#
# OTA updates replace the rootfs, wiping /etc (the systemd unit) and any
# xochitl-autostart change. Everything else lives under /home/root and survives.
# Run this once after each OS update. Idempotent. --dry-run supported.
set -eu

DRY_RUN=0
[ "${1:-}" = "--dry-run" ] && DRY_RUN=1

PROFILES_DIR="${RMP_PROFILES_DIR:-/home/root/profiles}"
UNIT_DIR="${RMP_UNIT_DIR:-/etc/systemd/system}"
SRC="${RMP_SRC:-$(unset CDPATH; cd -- "$(dirname -- "$0")/.." && pwd)}"
SHARE_DIR="${RMP_SHARE_DIR:-/home/root/.local/share/remarkable}"
CONFIG_DIR="${RMP_CONFIG_DIR:-/home/root/.config/remarkable}"

say() { printf '==> %s\n' "$1"; }

run() {
  if [ "$DRY_RUN" = "1" ]; then
    printf 'DRY-RUN: %s\n' "$*"
  else
    "$@"
  fi
}

say "Reapplying reMarkable Profiles after OS update (dry-run=$DRY_RUN)"

# Re-assert live symlinks (an update may have recreated real dirs).
say "Re-asserting live symlinks"
run ln -sfn "$PROFILES_DIR/active/xochitl" "$SHARE_DIR/xochitl"
run ln -sfn "$PROFILES_DIR/active/config" "$CONFIG_DIR"

# Reinstall the systemd gate.
say "Reinstalling systemd gate unit"
run cp "$SRC/systemd/rm-profile-pad.service" "$UNIT_DIR/rm-profile-pad.service"
run systemctl daemon-reload
run systemctl enable rm-profile-pad.service

# If you use the pre-xochitl boot model, re-disable xochitl autostart here:
# run systemctl disable xochitl

say "Done. Reboot to verify: systemctl reboot"
