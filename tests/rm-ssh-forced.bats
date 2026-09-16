#!/usr/bin/env bats
# Tests for the wifi SSH forced command. Runs against a scratch profiles dir
# with a stub engine that records its arguments, so it never switches anything.

setup() {
  FORCED="${BATS_TEST_DIRNAME}/../bin/rm-ssh-forced"
  TMP="$(mktemp -d "${BATS_TMPDIR:-/tmp}/rmsf.XXXXXX")"
  export RMP_PROFILES_DIR="$TMP/profiles"
  mkdir -p "$RMP_PROFILES_DIR/user1/xochitl" "$RMP_PROFILES_DIR/user1/config"
  mkdir -p "$RMP_PROFILES_DIR/user2/xochitl" "$RMP_PROFILES_DIR/user2/config"
  ln -s user1 "$RMP_PROFILES_DIR/active"
  CALLS="$TMP/calls"
  printf '#!/bin/sh\nprintf "%%s\\n" "$*" >> "%s"\n' "$CALLS" > "$RMP_PROFILES_DIR/rm-profile"
  chmod +x "$RMP_PROFILES_DIR/rm-profile"
}

teardown() {
  rm -rf "$TMP"
}

forced() {
  SSH_ORIGINAL_COMMAND="$1" "$FORCED"
}

assert_denied() {
  [ "$status" -ne 0 ]
  case "$output" in
    *denied*) ;;
    *) return 1 ;;
  esac
  [ ! -e "$CALLS" ]
}

@test "allows switch to an existing profile by the engine's full path" {
  run forced "/home/root/profiles/rm-profile switch user2"
  [ "$status" -eq 0 ]
  [ "$(cat "$CALLS")" = "switch user2" ]
}

@test "allows switch by the bare engine name" {
  run forced "rm-profile switch user1"
  [ "$status" -eq 0 ]
  [ "$(cat "$CALLS")" = "switch user1" ]
}

@test "allows any existing profile with a safe name, not a fixed list" {
  mkdir -p "$RMP_PROFILES_DIR/guest_2-b"
  run forced "rm-profile switch guest_2-b"
  [ "$status" -eq 0 ]
  [ "$(cat "$CALLS")" = "switch guest_2-b" ]
}

@test "tolerates surrounding whitespace and a trailing newline" {
  run forced "  rm-profile   switch	user2
"
  [ "$status" -eq 0 ]
  [ "$(cat "$CALLS")" = "switch user2" ]
}

@test "denies a safe name with no profile directory" {
  run forced "rm-profile switch ghost"
  assert_denied
}

@test "denies a safe name that is a file, not a directory" {
  touch "$RMP_PROFILES_DIR/stray"
  run forced "rm-profile switch stray"
  assert_denied
}

@test "denies the active symlink" {
  run forced "rm-profile switch active"
  assert_denied
}

@test "allows a 32-character name" {
  name="$(printf 'a%.0s' $(seq 1 32))"
  mkdir -p "$RMP_PROFILES_DIR/$name"
  run forced "rm-profile switch $name"
  [ "$status" -eq 0 ]
  [ "$(cat "$CALLS")" = "switch $name" ]
}

@test "denies unsafe names even when the directory exists" {
  long="$(printf 'a%.0s' $(seq 1 33))"
  for name in User1 user.1 -user1 _user1 "$long"; do
    mkdir -p -- "$RMP_PROFILES_DIR/$name"
    run forced "rm-profile switch $name"
    assert_denied
  done
}

@test "denies shell metacharacters and path tricks" {
  for name in 'user1;reboot' 'user1&&reboot' 'user1|sh' '$(reboot)' '`reboot`' \
    '../profiles/user1' 'user1/' '~user1'; do
    run forced "rm-profile switch $name"
    assert_denied
  done
}

@test "does not expand glob characters in the request" {
  mkdir -p "$TMP/cwd/user1"
  cd "$TMP/cwd"
  run forced "rm-profile switch use?"
  assert_denied
  run forced "rm-profile switch *"
  assert_denied
}

@test "denies extra arguments" {
  run forced "rm-profile switch user1 now"
  assert_denied
  run forced "rm-profile switch user1 user2"
  assert_denied
}

@test "denies anything but the switch subcommand" {
  for cmd in "rm-profile list" "rm-profile status" "rm-profile create user3" \
    "rm-profile set-pin user1" "rm-profile hash user1" "reboot switch user1" \
    "sh -c reboot" "switch user1" "user1"; do
    run forced "$cmd"
    assert_denied
  done
}

@test "denies an empty or missing command" {
  run forced ""
  assert_denied
  run env -u SSH_ORIGINAL_COMMAND "$FORCED"
  assert_denied
}
