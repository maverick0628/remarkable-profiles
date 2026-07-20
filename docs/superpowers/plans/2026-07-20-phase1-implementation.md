# reMarkable Profiles — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Phase 1 build — a PIN-switched two-profile system for reMarkable 2:
a POSIX shell switch engine, a Rust PIN pad (host-tested logic + cross-compiled binary),
a systemd boot gate, and a device install runbook.

**Architecture:** A profile is two directories under `/home/root/profiles/<name>/`
(`xochitl/` documents, `config/` settings). A single `active` symlink pivots which
profile the live paths resolve to. Switching = stop xochitl → repoint `active` → start
xochitl. A custom fullscreen PIN pad runs at boot, maps entered PIN → profile via salted
hashes, and starts xochitl only after a correct PIN.

**Tech Stack:** POSIX `sh` (busybox `ash` on device), `bats` + `shellcheck` for shell
tests/lint, Rust (`sha2` for hashing, `libremarkable` for the framebuffer UI), `cross`
for `armv7-unknown-linux-gnueabihf`, systemd.

## Global Constraints

- **Device:** reMarkable 2 only. No Toltec, no launcher, no third-party package manager.
- **Shell:** all device scripts POSIX `sh` (busybox `ash`) — no bashisms. Must pass
  `shellcheck -s sh`.
- **Non-destructive:** a switch never deletes or overwrites the other profile's data.
- **Source of truth under `/home/root`:** binaries, scripts, units, profile data — so it
  survives OTA rootfs replacement. A `reapply.sh` reinstalls the `/etc` bits.
- **Security framing:** convenience + basic privacy, not a security boundary. Never
  describe it otherwise in code comments or docs.
- **Canonical PIN hash scheme (both shell and Rust MUST match exactly):**
  `hash = lowercase_hex( sha256( salt_ascii concatenated with pin_ascii ) )`.
  `pins.conf` format: one `name:salt:hash` record per line.
- **Testability:** the engine reads paths/commands from env vars with production defaults
  so bats can run it against a scratch dir with stubbed start/stop commands.

---

## File Structure

```
bin/rm-profile                     # POSIX sh switch engine (list/status/switch/create/set-pin)
pad/Cargo.toml                     # Rust workspace
pad/core/                          # rmprofile-core: pure logic, host-tested
  src/lib.rs                       #   parse_pins, hash_pin, match_pin
pad/pad/                           # rm-profile-pad: libremarkable framebuffer bin (armv7)
  src/main.rs
pad/Cross.toml                     # cross target config
systemd/rm-profile-pad.service     # boot gate unit
scripts/rm-profile-setup           # device install: backup -> migrate -> create kid -> install
scripts/reapply.sh                 # post-OTA reinstall of /etc bits + symlink re-assert
tests/rm-profile.bats              # bats tests for the engine
tests/hash-consistency.bats        # asserts shell hash == known vector (matches Rust)
docs/INSTALL.md                    # on-device runbook (Phase 0 spike + Phase 1 install)
```

---

### Task 1: Rust workspace + core logic crate (host-tested)

**Files:**
- Create: `pad/Cargo.toml`, `pad/core/Cargo.toml`, `pad/core/src/lib.rs`
- Create: `pad/Cross.toml`

**Interfaces produced:**
- `rmprofile_core::hash_pin(salt: &str, pin: &str) -> String` — lowercase hex sha256 of
  `salt` concatenated with `pin`.
- `rmprofile_core::PinEntry { name: String, salt: String, hash: String }`
- `rmprofile_core::parse_pins(contents: &str) -> Vec<PinEntry>` — skips blank lines and
  `#` comments; splits on `:` into exactly 3 fields.
- `rmprofile_core::match_pin<'a>(entries: &'a [PinEntry], pin: &str) -> Option<&'a str>`
  — returns the name of the first entry whose `hash_pin(entry.salt, pin) == entry.hash`.

**Canonical test vector (shared with the shell engine):**
`hash_pin("cafebabe", "1234") == "b8a...<computed>"` — the exact value is computed in
Step 3 and copied verbatim into `tests/hash-consistency.bats` in Task 2.

- [ ] **Step 1: Write failing tests** in `pad/core/src/lib.rs` (`#[cfg(test)]`):
  determinism (`hash_pin` twice equal), sensitivity (different pin → different hash),
  `parse_pins` skips comments/blanks and parses 3 fields, `match_pin` returns correct
  name and `None` on wrong pin.
- [ ] **Step 2: Run** `cd pad && cargo test -p rmprofile-core` → FAIL (not implemented).
- [ ] **Step 3: Implement** `lib.rs` using the `sha2` crate; print the canonical vector
  once (`hash_pin("cafebabe","1234")`) and record it in this plan + Task 2.
- [ ] **Step 4: Run** `cargo test -p rmprofile-core` → PASS.
- [ ] **Step 5: Commit** `feat(core): PIN hashing, parsing, and matching with tests`.

### Task 2: rm-profile switch engine + bats tests (TDD)

**Files:**
- Create: `bin/rm-profile`, `tests/rm-profile.bats`, `tests/hash-consistency.bats`

**Env contract (defaults are production values):**
- `RMP_PROFILES_DIR` (default `/home/root/profiles`)
- `RMP_STOP_CMD` (default `systemctl stop xochitl`)
- `RMP_START_CMD` (default `systemctl start xochitl`)
- `RMP_STATUS_CMD` (default `systemctl is-active xochitl`)

**Interfaces produced (CLI):**
- `rm-profile list` — one line per profile, `* ` prefix on active.
- `rm-profile status` — active name, xochitl state, symlink health; exit 0 if healthy.
- `rm-profile switch <name>` — validates target has `xochitl/` and `config/`; no-op with
  message if already active; else stop → sync → `ln -sfn <name> active` → start.
- `rm-profile create <name>` — scaffolds `<dir>/xochitl/` and `<dir>/config/`; refuses if
  it exists.
- `rm-profile set-pin <name> <pin>` — generates 16-byte hex salt, writes/replaces the
  `name:salt:hash` record in `<profiles>/pins.conf` (0600). `<pin>` optional → read from
  stdin for interactive use.

- [ ] **Step 1: Write failing bats** (`tests/rm-profile.bats`): setup creates a temp
  `RMP_PROFILES_DIR` with `duncan/{xochitl,config}` + `active->duncan`, exports
  `RMP_STOP_CMD=: RMP_START_CMD=: RMP_STATUS_CMD='echo active'`. Cases: `list` marks
  active; `create kid` makes dirs; `switch kid` repoints `active` and calls stop/start
  (assert via a stop/start that `touch`es a marker file); `switch` to missing profile
  fails non-zero and leaves `active` unchanged; `switch` to current is a no-op; `set-pin`
  writes a parseable `name:salt:hash` line with 0600 perms.
- [ ] **Step 2: Run** `bats tests/rm-profile.bats` → FAIL (script absent).
- [ ] **Step 3: Implement** `bin/rm-profile` (POSIX sh, `set -eu`). Hash helper tries
  `sha256sum` then `shasum -a 256`; salt via `head -c16 /dev/urandom | od -An -tx1 |
  tr -d ' \n'`. `switch` uses `ln -sfn` for the atomic repoint.
- [ ] **Step 4: Run** `bats tests/rm-profile.bats` → PASS.
- [ ] **Step 5: Add** `tests/hash-consistency.bats`: assert the engine's hash of
  salt `cafebabe` + pin `1234` equals the canonical vector from Task 1 (guards shell⇄Rust
  drift). Run → PASS.
- [ ] **Step 6:** `shellcheck -s sh bin/rm-profile` → no findings.
- [ ] **Step 7: Commit** `feat(engine): rm-profile switch engine with bats + shellcheck`.

### Task 3: PIN pad framebuffer binary (cross-compiled)

**Files:**
- Create: `pad/pad/Cargo.toml`, `pad/pad/src/main.rs`

**Consumes:** `rmprofile-core` (Task 1). **Produces:** an `armv7` binary
`rm-profile-pad`.

Behavior: draw a numeric keypad via `libremarkable`, read touch input, accumulate a PIN,
call `match_pin` against `/home/root/profiles/pins.conf`; on match to a **different**
profile run `rm-profile switch <name>` then `systemctl start xochitl`; on match to the
**active** profile just `systemctl start xochitl`; on mismatch clear and show retry.
Keep the binary thin — all pure logic lives in `rmprofile-core`.

- [ ] **Step 1:** Write `main.rs` (keypad layout, input loop, delegates to core + shells
  out to `rm-profile`/`systemctl`). UI is validated on-device (Task 6 runbook), not here.
- [ ] **Step 2: Verify it compiles for the target:**
  `cd pad && cross build --release --target armv7-unknown-linux-gnueabihf -p rm-profile-pad`
  → builds successfully. (Runtime UI testing is on-device only.)
- [ ] **Step 3: Commit** `feat(pad): libremarkable PIN pad binary, cross-compiles for rM2`.

### Task 4: systemd gate unit

**Files:**
- Create: `systemd/rm-profile-pad.service`

Unit: `Type=oneshot`, `Before=xochitl.service`, runs `/home/root/profiles/rm-profile-pad`
which blocks until a correct PIN, then starts xochitl. Installed by setup, which also runs
`systemctl disable xochitl` so nothing starts xochitl except the pad. Exact `After=`/target
ordering is tuned on-device (documented in INSTALL.md — framebuffer/input readiness at
boot is device-specific).

- [ ] **Step 1:** Write the unit file with the ordering above and comments explaining the
  disable-xochitl-autostart requirement.
- [ ] **Step 2:** `systemd-analyze verify systemd/rm-profile-pad.service` if available;
  otherwise a syntax read-through (macOS has no systemd — verification is on-device).
- [ ] **Step 3: Commit** `feat(systemd): boot gate unit for the PIN pad`.

### Task 5: setup, migration, backup, and reapply scripts

**Files:**
- Create: `scripts/rm-profile-setup`, `scripts/reapply.sh`

`rm-profile-setup` (POSIX sh, `set -eu`, device-only, supports `--dry-run`):
mandatory verified backup of `~/.local/share/remarkable` + `~/.config/remarkable` →
`/home/root/profiles-backup-<date>` with an explicit "copy this off-device" prompt; stop
xochitl; migrate current data into `profiles/duncan/`; create live dir-symlinks
(`xochitl` and the `config` dir, NOT the single conf file); `rm-profile create kid`;
`set-pin` for both; remove any native passcode from migrated configs (best-effort, with a
manual-confirm note since the exact key is confirmed in Phase 0); install the unit,
`systemctl disable xochitl`, enable the pad; prompt to reboot.

`reapply.sh`: re-copy the unit into `/etc/systemd/system`, re-`disable xochitl`, re-assert
symlinks — for use after any OTA update.

- [ ] **Step 1:** Write both scripts defensively; `--dry-run` prints every mutating action
  without executing.
- [ ] **Step 2:** `shellcheck -s sh scripts/rm-profile-setup scripts/reapply.sh` → clean.
- [ ] **Step 3:** Run `scripts/rm-profile-setup --dry-run` against a scratch HOME to prove
  it plans the right actions without touching anything real.
- [ ] **Step 4: Commit** `feat(setup): install/migration/backup + post-OTA reapply`.

### Task 6: On-device install runbook

**Files:**
- Create: `docs/INSTALL.md`

Covers: enabling SSH, **Phase 0 spike** (confirm `xochitl.conf` PIN/cloud key names;
confirm stop→swap-dirs→start loads a second profile; confirm native-lock disable key;
find the auto-update pause method for the installed OS version), the backup-and-verify
step, cross-building and `scp`-ing the pad, running `rm-profile-setup`, first-boot
verification, and the post-update `reapply.sh` step. Explicit rM2-only and
not-a-security-boundary warnings.

- [ ] **Step 1:** Write `docs/INSTALL.md` with exact commands and the Phase 0 checklist.
- [ ] **Step 2: Commit** `docs: on-device install runbook with Phase 0 spike`.

### Task 7: Finalize — full green run, PR, merge

- [ ] **Step 1:** Run the whole suite: `shellcheck` (all scripts), `bats tests/`,
  `cargo test`, `cross build --release`. Capture output.
- [ ] **Step 2:** Update `README.md` status + usage; commit.
- [ ] **Step 3:** Push branch, open PR with the green evidence, merge as solo owner,
  delete branch.

---

## Self-Review

**Spec coverage:** data model → Tasks 1/2/5; switch engine → Task 2; PIN pad → Tasks 1/3;
boot gate (Phase 1) → Task 4; setup/migration/backup → Task 5; OS-update resilience →
Task 5 (`reapply.sh`) + Task 6; threat model + rM2-only → Global Constraints + Task 6;
build/test plan → every task's steps + Task 7. Phase 2 (every-wake resume hook) is
intentionally out of scope. No gaps.

**Placeholder scan:** no TBD/TODO. Device-only verification steps (systemd behavior, pad
UI runtime, setup mutation) are labeled as on-device because they are physically
un-runnable off-device — this is a hardware constraint, not a placeholder.

**Type consistency:** `hash_pin(salt, pin)`, `parse_pins`, `match_pin`, `PinEntry`, and
the `name:salt:hash` / canonical hash scheme are used identically across the core crate,
the engine, the consistency test, and the pad. The env-var contract
(`RMP_PROFILES_DIR/STOP/START/STATUS`) is consistent between engine and tests.
