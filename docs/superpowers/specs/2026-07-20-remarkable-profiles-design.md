# reMarkable Profiles — Design Spec

**Date:** 2026-07-20
**Status:** Approved design, pre-implementation
**Target device:** reMarkable 2 (rM2) only

## Problem

A single reMarkable 2 is shared between two people (owner and a child). The tablet
natively binds one device to one account with one set of notebooks and one PIN. There
is no built-in concept of user profiles, and no maintained community tool provides one
(the only prior attempt, `Riebart/reMarkable-Profiles`, is an abandoned README-only
stub).

We want distinct user profiles that switch by PIN: power on or wake, type your PIN, land
in your own notebooks. The owner's profile keeps its reMarkable cloud sync. The child's
profile is local-only.

## Goals

- Two isolated profiles on one rM2: separate notebooks, separate PIN, separate cloud
  identity.
- Owner profile retains existing documents and cloud sync (reMarkable Connect).
- Child profile is local-only: its own notebooks, its own PIN, no cloud account, no
  second Connect subscription.
- Switching is driven by PIN entry (a custom lock screen), simple enough for a child to
  operate unaided.
- Non-destructive: switching never risks the other profile's data.
- Survives reMarkable OS (OTA) updates with a documented reapply step.

## Non-goals

- **Not** a security boundary. This is household convenience plus basic privacy. Anyone
  with USB or SSH access can read both profiles' data — already true of any rooted
  reMarkable. Do not present this as protecting data against a technical adversary.
- No separate wifi per profile (wifi is system-level, stays shared).
- No separate templates per profile (templates stay shared by design — see Data model).
- No cloud sync for the child profile.
- No parental controls or content filtering.
- No support for reMarkable 1 or the Paper Pro family in this version. rM2 only.

## Constraints and assumptions

- rM2 exposes SSH root access out of the box (USB `10.11.99.1` or wifi IP; root password
  under Settings). No Toltec, no launcher, and no third-party package manager is
  required or used — this deliberately avoids Toltec's maintenance decline and its
  OS-version soft-brick ceiling.
- On-device shell is busybox `ash`; all device scripts must be POSIX `sh`, not bash.
- xochitl is closed-source and cannot be patched. We work around it, never inside it.
- Exact config key names (PIN field, cloud token field in `xochitl.conf`) are to be
  confirmed on-device during the first build spike; the design does not depend on their
  specific names, only on swapping whole directories.

## Key insight

The switch engine is trivial; the gate is the only hard part.

A profile is two directories on disk. Switching = stop `xochitl`, repoint those
directories, start `xochitl`. Roughly 30 lines of shell. Once a profile's config is
loaded, **xochitl's own native lock uses that profile's PIN for free** — so per-profile
PIN validation is already solved. The only thing xochitl cannot do is choose *which*
profile to load from the PIN, because it only ever has one config loaded. Everything
hard in this project is about putting a gate in front of xochitl that picks the profile.

## Architecture

### Data model

Per-profile state lives under `/home/root/profiles/<name>/`:

- `xochitl/` — documents (mirror of `~/.local/share/remarkable/xochitl/`)
- `config/` — configuration (mirror of `~/.config/remarkable/`), containing that
  profile's PIN and, for the owner, the cloud token

Live paths become **directory** symlinks resolved through a single active pointer:

- `~/.local/share/remarkable/xochitl`  →  `/home/root/profiles/active/xochitl`
- `~/.config/remarkable`               →  `/home/root/profiles/active/config`
- `/home/root/profiles/active`         →  `duncan` | `kid`

Symlinking the config **directory** (not the single `xochitl.conf` file) avoids the
write-temp-then-rename clobber that would silently replace a file-level symlink with a
real file. If xochitl is observed to misbehave with dir symlinks, fall back to
`mount --bind` re-established at boot.

**Isolated:** notebooks, PIN, cloud account, UI preferences.
**Shared:** wifi (system-level, untouched) and custom templates. Templates live at
`~/.local/share/remarkable/templates/`, a sibling of `xochitl/`, so swapping only the
`xochitl/` subdirectory leaves templates common to both profiles automatically.

### Component 1 — Switch engine (`rm-profile`)

POSIX `sh` script, subcommands:

- `rm-profile list` — profiles and which is active
- `rm-profile status` — active profile, xochitl state, sanity checks
- `rm-profile switch <name>` — `systemctl stop xochitl` → `sync` → repoint
  `profiles/active` → `systemctl start xochitl`
- `rm-profile create <name>` — scaffold a new empty profile (empty `xochitl/`, fresh
  `config/` with a set PIN and no cloud token)

Safety rules: never runs a swap while xochitl holds the files (stop first, `sync`
between); never deletes a profile as part of a switch; verifies the target profile
exists and is well-formed before touching the active pointer. `rm-sync` starts and stops
with xochitl via systemd, so cloud follows the active profile with no extra handling.

This engine is built and tested first, in isolation. It is independently useful (switch
over SSH) and is the foundation the pad calls.

### Component 2 — PIN pad (`rm-profile-pad`)

A small fullscreen binary — Rust, `libremarkable` crate, single static `armv7` binary,
cross-compiled off-device. Responsibilities:

1. Draw a numeric keypad to the framebuffer, capture touch input.
2. Read salted PIN hashes from a root-only `/home/root/profiles/pins.conf`, map the
   entered PIN → profile.
3. On match:
   - entered PIN = **currently active** profile → dismiss, reveal xochitl (no restart)
   - entered PIN = **other** profile → call `rm-profile switch` → restart xochitl
4. On mismatch → standard wrong-PIN feedback and retry.

xochitl's native lock is disabled in both profiles so the pad is the single gate (no
double-PIN prompt). The pad's non-UI logic (hashing, PIN→profile mapping, wrong-PIN
handling) is unit-testable off-device; the framebuffer UI is tested on the tablet.

### Component 3 — Gate trigger (Phase 1)

The pad runs as a systemd unit (`rm-profile-pad.service`) that gates xochitl:

- xochitl's automatic start is disabled; the pad service starts at boot and invokes
  `systemctl start xochitl` only after a correct PIN.
- **Phase 1 behavior:** the pad appears at **cold boot** and on a deliberate switch
  action. Within a single session, sleep/wake stays in the active profile. Switching
  users is an explicit act: a power cycle (child-friendly: off → on → type PIN, ~15s) or
  a phone/SSH trigger for the owner.

Phase 1 is the shipping target. It is fully on-device, low-risk, and does not depend on
winning any race with xochitl.

### Deferred — Seamless every-wake gate (Phase 2)

A systemd resume hook (`/lib/systemd/system-sleep/`) that shows the pad on **every wake**
so it is both switch point and lock at every unlock — the fully seamless "auto by PIN"
experience. **Primary technical risk:** the pad must draw before xochitl repaints
content on resume; that timing may be unreliable. Deferred to a post-Phase-1 spike. If it
proves unreliable, Phase 1 stands on its own. Not committed for the first release.

## Setup, migration, backup

1. **Backup first.** Full off-device `rsync` of `~/.local/share/remarkable/` and
   `~/.config/remarkable/` before any change. Nothing proceeds without a verified backup.
2. **Migrate.** Move current data into `profiles/duncan/` (documents + config, including
   the live cloud token). Establish the symlinks and active pointer.
3. **Create child profile.** `rm-profile create kid` — empty notebooks, a set PIN, no
   cloud token (never paired ⇒ never syncs).
4. **Install** the `rm-profile` engine, the pad binary, `pins.conf`, and the systemd
   unit; disable xochitl native lock in both profiles; disable xochitl auto-start.

## OS update resilience

reMarkable OTA updates replace the rootfs (A/B partitions), which can wipe changes under
`/etc` — including installed systemd units.

- The **source of truth** (pad binary, `rm-profile`, unit files, `pins.conf`, all
  profile data) lives under `/home/root`, which persists across OTA updates.
- Ship a `reapply.sh` that reinstalls units and re-disables the native lock / auto-start.
- **Recommend pausing auto-updates** so updates are deliberate, each followed by
  `reapply.sh`. Document the reapply step prominently in the README.

## Threat model

- **In scope:** casual privacy between household members (child does not see owner's
  notebooks and vice versa); convenience of one shared device.
- **Out of scope:** any adversary with USB or SSH access. Root can read every profile.
  The PIN gate is a convenience lock, not encryption. State this plainly in user-facing
  docs so no one over-trusts it.

## Build and test plan

- **Engine:** `bats` tests for `rm-profile` (create/list/switch/status; asserts document
  isolation, no data loss, cloud token present only in owner profile). Runnable against a
  scratch dir off-device and validated on-device.
- **Pad logic:** Rust unit tests for PIN hashing and PIN→profile mapping.
- **Pad UI:** manual on-device testing (keypad render, touch, wrong-PIN, launch).
- **Cross-compile:** `armv7-unknown-linux-gnueabihf` via the `libremarkable` toolchain;
  deploy over SSH (`scp`).
- **Resilience:** verify active profile persists across reboot; document and dry-run the
  post-update reapply.

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Config dir symlink clobbered by xochitl rewrite | Med | Symlink the dir not the file; bind-mount fallback |
| OTA update wipes units / re-enables native lock | Med | Source of truth in `/home/root`; `reapply.sh`; pause auto-update |
| Phase 2 resume race unreliable | Med (Phase 2 only) | Deferred; spiked separately; Phase 1 independent |
| Data loss during migration | High | Mandatory verified off-device backup before any change |
| Wrong on-device config key assumptions | Low | Swap whole dirs, not individual keys; confirm keys in build spike |

## Phased roadmap

- **Phase 0 — Spike:** confirm on-device paths/keys; verify stop → swap dirs → start
  cleanly loads a second profile; verify native per-profile PIN works post-swap.
- **Phase 1 — Ship:** switch engine + PIN pad + boot gate + setup/migration/backup +
  reapply. Power-cycle-to-switch UX.
- **Phase 2 — Deferred:** every-wake resume hook for seamless switching.

## Open questions

- Exact `xochitl.conf` key names for PIN and cloud token (resolved in Phase 0).
- Confirm rM2 OS version and current SSH access method before install.
- Method to pause auto-updates on the owner's current OS version (resolved in Phase 0).

## Outcome addendum (2026-07-20)

Installed on-device the same day. Phase 0 found the target rM2 running **OS 3.27.3.0**
(Codex Linux, scarthgap). Findings that changed the plan:

- **The PIN pad is not viable on this OS.** rM2 draws through `rm2fb`, which patches
  xochitl at per-version memory offsets; its table ends at **3.3.2.1666**. `/dev/fb0`
  exists (`mxs-lcdif`, 32bpp, packed 260×23936) but is xochitl-owned and can't be driven
  directly without documented format/refresh handling. So auto-by-PIN is blocked on
  OS > 3.3 — it would require reverse-engineering the framebuffer for a 2026 build.
- **Shipped the engine + handoff model instead.** Two isolated profiles (`duncan`
  cloud-synced, `kid` local-only), switched via `rm-profile switch` triggered from an
  iPhone shortcut / SSH. Each unlocks with its own native PIN. Confirmed on-device:
  kid shows an empty library and has zero cloud tokens; owner profile unchanged.
- The child config is seeded from a sanitized copy of the owner config (cloud tokens +
  passcode stripped) so it is onboarded and local-only without forced account sign-in.

The pad code (`pad/`, `systemd/`, `scripts/rm-profile-setup`) is retained for OS ≤ 3.3 and
as a future path. The shipped path is `scripts/rm-profile-migrate` + `scripts/rm-switch`.
