#!/usr/bin/env bats
# Tests for the rm-profile switch engine. Runs against a scratch profiles dir
# with stubbed xochitl start/stop commands, so it never touches a real device.

setup() {
  RMP="${BATS_TEST_DIRNAME}/../bin/rm-profile"
  TMP="$(mktemp -d "${BATS_TMPDIR:-/tmp}/rmp.XXXXXX")"
  export RMP_PROFILES_DIR="$TMP/profiles"
  mkdir -p "$RMP_PROFILES_DIR/duncan/xochitl" "$RMP_PROFILES_DIR/duncan/config"
  ln -s duncan "$RMP_PROFILES_DIR/active"
  export RMP_STOP_CMD="touch $TMP/stopped"
  export RMP_START_CMD="touch $TMP/started"
  export RMP_STATUS_CMD="echo active"
}

teardown() {
  rm -rf "$TMP"
}

active_target() {
  basename "$(readlink "$RMP_PROFILES_DIR/active")"
}

@test "list marks the active profile with an asterisk" {
  mkdir -p "$RMP_PROFILES_DIR/kid/xochitl" "$RMP_PROFILES_DIR/kid/config"
  run "$RMP" list
  [ "$status" -eq 0 ]
  [[ "$output" == *"* duncan"* ]]
  [[ "$output" == *"  kid"* ]]
  [[ "$output" != *"* kid"* ]]
}

@test "create scaffolds a new empty profile" {
  run "$RMP" create kid
  [ "$status" -eq 0 ]
  [ -d "$RMP_PROFILES_DIR/kid/xochitl" ]
  [ -d "$RMP_PROFILES_DIR/kid/config" ]
}

@test "create refuses to clobber an existing profile" {
  run "$RMP" create duncan
  [ "$status" -ne 0 ]
}

@test "create rejects the reserved name 'active'" {
  run "$RMP" create active
  [ "$status" -ne 0 ]
}

@test "switch repoints active and runs stop then start" {
  "$RMP" create kid
  run "$RMP" switch kid
  [ "$status" -eq 0 ]
  [ "$(active_target)" = "kid" ]
  [ -f "$TMP/stopped" ]
  [ -f "$TMP/started" ]
}

@test "switch to a missing profile fails and leaves active unchanged" {
  run "$RMP" switch ghost
  [ "$status" -ne 0 ]
  [ "$(active_target)" = "duncan" ]
  [ ! -f "$TMP/stopped" ]
}

@test "switch to a malformed profile (no config) fails" {
  mkdir -p "$RMP_PROFILES_DIR/broken/xochitl"
  run "$RMP" switch broken
  [ "$status" -ne 0 ]
  [ "$(active_target)" = "duncan" ]
}

@test "switch to the already-active profile is a no-op success" {
  run "$RMP" switch duncan
  [ "$status" -eq 0 ]
  [ ! -f "$TMP/stopped" ]
}

@test "status reports the active profile and is healthy" {
  run "$RMP" status
  [ "$status" -eq 0 ]
  [[ "$output" == *"duncan"* ]]
}

@test "status is unhealthy when the active symlink is dangling" {
  rm "$RMP_PROFILES_DIR/active"
  ln -s ghost "$RMP_PROFILES_DIR/active"
  run "$RMP" status
  [ "$status" -ne 0 ]
}

@test "set-pin writes a parseable name:salt:hash record" {
  run "$RMP" set-pin kid 4321
  [ "$status" -eq 0 ]
  [ -f "$RMP_PROFILES_DIR/pins.conf" ]
  grep -Eq '^kid:[0-9a-f]{32}:[0-9a-f]{64}$' "$RMP_PROFILES_DIR/pins.conf"
}

@test "set-pin replaces an existing record rather than duplicating it" {
  "$RMP" set-pin kid 1111
  "$RMP" set-pin kid 2222
  [ "$(grep -c '^kid:' "$RMP_PROFILES_DIR/pins.conf")" -eq 1 ]
}

@test "set-pin writes pins.conf with 0600 permissions" {
  "$RMP" set-pin kid 4321
  perms="$(stat -f '%Lp' "$RMP_PROFILES_DIR/pins.conf" 2>/dev/null || stat -c '%a' "$RMP_PROFILES_DIR/pins.conf")"
  [ "$perms" = "600" ]
}
