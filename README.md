# reMarkable Profiles

Distinct, switchable user profiles on a single **reMarkable 2** — separate notebooks,
separate PIN, separate cloud account. One profile keeps reMarkable cloud sync; another is
local-only. Ideal for sharing a tablet with a child.

No Toltec, no launcher. A small POSIX-shell switch engine installed over SSH.

> **This is convenience and basic privacy, not security.** Anyone with USB or SSH access
> can read every profile — as on any rooted reMarkable. Don't rely on it to protect data.

## How it works

A profile is two directories — `xochitl/` (documents) and `config/` (settings, PIN, cloud
token). A single `active` symlink pivots which the tablet uses. Switching = stop xochitl →
repoint `active` → start xochitl (~3-5s). After a switch, xochitl's own lock screen uses
that profile's native PIN, so per-profile locking is automatic.

**Selecting a profile** depends on your OS version:

- **Handoff switching (shipped, works on all current OS).** Trigger a switch when you hand
  the tablet over — a phone shortcut or an SSH one-liner (`rm-switch`). Each person then
  unlocks with their own native PIN. This is what runs on reMarkable OS newer than ~3.3.
- **Auto-by-PIN pad (OS ≤ 3.3 only).** A custom boot PIN pad that maps the entered PIN to a
  profile automatically. It draws through `rm2fb`, whose support tops out at OS 3.3.2.1666,
  so it does **not** work on newer builds (see [Status](#status)). Code lives in `pad/`.

## Status

**Deployed** in the handoff model on a reMarkable 2 running OS 3.27.3.0 (2026-07-20). Two
profiles, switched by an iPhone shortcut / SSH.

The auto-by-PIN pad is **blocked on OS > 3.3**: `rm2fb` patches xochitl at per-version
memory offsets and has no entries beyond 3.3.2.1666, and the rM2's packed framebuffer can't
be driven directly without it. The pad remains in-repo for OS ≤ 3.3 and as a future path if
framebuffer access is solved for newer builds.

## Install (engine + handoff)

See [`docs/INSTALL.md`](docs/INSTALL.md). In short, from a Mac with the tablet on SSH:

```
scp bin/rm-profile root@10.11.99.1:/home/root/profiles/rm-profile   # deploy engine
scp -r bin scripts root@10.11.99.1:/home/root/remarkable-profiles/   # + migrate script
ssh root@10.11.99.1 'sh /home/root/remarkable-profiles/scripts/rm-profile-migrate --dry-run'
ssh root@10.11.99.1 'sh /home/root/remarkable-profiles/scripts/rm-profile-migrate'
```

Back up first (the script also backs up; copy it off-device).

## Switching

```
scripts/rm-switch kid                 # switch, over USB (default host)
RM_HOST=192.168.86.62 scripts/rm-switch duncan   # over wifi
```

Or an iOS Shortcut running `/home/root/profiles/rm-profile switch <name>` over SSH — see
`docs/INSTALL.md`.

## Layout

```
bin/rm-profile              switch engine (list/status/switch/create/set-pin)
scripts/rm-profile-migrate  engine-only install (shipped model)
scripts/rm-switch           Mac/phone-side switch trigger
scripts/rm-profile-setup    full install incl. PIN pad (OS <= 3.3)
scripts/reapply.sh          post-OTA reapply (pad model)
pad/                        Rust PIN pad — core logic (host-tested) + ARMv7 binary
systemd/                    boot gate unit (pad model)
tests/                      bats suite
docs/INSTALL.md             on-device runbook
```

## Build & test

```
cd pad && cargo test -p rmprofile-core        # core logic (12 tests)
bats tests/                                    # engine + hash-consistency (14 tests)
shellcheck -s sh bin/rm-profile scripts/*      # lint
```

## Supported

reMarkable 2 only. Not reMarkable 1, not the Paper Pro family.
