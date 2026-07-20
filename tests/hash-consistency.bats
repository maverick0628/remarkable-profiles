#!/usr/bin/env bats
# Guards against drift between the shell engine's hash helper and
# rmprofile-core::hash_pin. Both must produce this exact value for the
# canonical (salt, pin) pair, or the pad and the engine will disagree about
# which PIN maps to which profile.

setup() {
  RMP="${BATS_TEST_DIRNAME}/../bin/rm-profile"
}

@test "shell hash of the canonical vector matches the Rust core constant" {
  run "$RMP" hash cafebabe 1234
  [ "$status" -eq 0 ]
  [ "$output" = "e35ced642ebca92d6bef21d6581b8d6f9a2a60037cc5689e7127006a8104a977" ]
}
