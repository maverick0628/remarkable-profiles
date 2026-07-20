# reMarkable Profiles

Distinct, PIN-switched user profiles on a single **reMarkable 2**. Power on, type your
PIN, land in your own notebooks. One profile keeps reMarkable cloud sync; another is
local-only — ideal for sharing a tablet with a child.

No Toltec, no launcher. A small custom PIN pad plus a POSIX shell switch engine, installed
over SSH.

> **This is convenience and basic privacy, not security.** Anyone with USB or SSH access
> can read every profile — as on any rooted reMarkable. Don't rely on it to protect data.

## Status

Phase 1 code complete and tested off-device; on-device install pending your tablet.

- **Switch engine** (`bin/rm-profile`) — POSIX sh, 14 bats tests, shellcheck clean.
- **PIN pad** (`pad/`) — Rust; host-tested decision logic (`rmprofile-core`), framebuffer
  binary cross-compiles to ARMv7.
- **Install** (`scripts/`, `systemd/`) — backup/migrate/reapply, boot gate unit.
- **Runbook** — [`docs/INSTALL.md`](docs/INSTALL.md) covers the on-device steps and the
  rm2fb bring-up (the main device-side unknown).

Design and plan of record:
[spec](docs/superpowers/specs/2026-07-20-remarkable-profiles-design.md) ·
[plan](docs/superpowers/plans/2026-07-20-phase1-implementation.md).

## How it works

A profile is two directories (`xochitl/` documents + `config/` settings, PIN, cloud
token). A single `active` symlink pivots which the tablet uses. Switching = stop xochitl →
repoint `active` → start xochitl. The PIN pad runs at boot, maps the entered PIN to a
profile via salted hashes, and starts xochitl only after a correct PIN.

## Layout

```
bin/rm-profile         switch engine (list/status/switch/create/set-pin)
pad/core               PIN hashing/parsing/matching + decision logic (host-tested)
pad/pad                libremarkable PIN pad binary (ARMv7)
systemd/               boot gate unit
scripts/               setup/migration/backup + post-OTA reapply
tests/                 bats suite
docs/INSTALL.md        on-device runbook
```

## Build & test

```
cd pad && cargo test -p rmprofile-core        # core logic
bats tests/                                    # engine + hash-consistency
shellcheck -s sh bin/rm-profile scripts/*      # lint
# ARMv7 pad build: see docs/INSTALL.md
```

## Supported

reMarkable 2 only. Not reMarkable 1, not the Paper Pro family.
