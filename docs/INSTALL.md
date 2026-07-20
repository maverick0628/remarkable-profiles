# Install runbook — reMarkable 2

This is the on-device half of the project. The code is built and tested off-device;
these steps run against your actual tablet over SSH.

> **reMarkable 2 only.** Not rM1, not Paper Pro.
>
> **This is convenience and basic privacy, not security.** Anyone with USB or SSH access
> can read every profile. Do not rely on it to protect sensitive data.
>
> **Back up first, and copy the backup off the device.** Migration moves your real
> notebooks. A verified off-device backup is non-negotiable.

## What you need

- A reMarkable 2 with SSH access. Get the root password from
  **Settings → General → Help → Copyrights and licenses** (bottom), and the USB IP is
  usually `10.11.99.1`. Test: `ssh root@10.11.99.1`.
- This repo checked out on your Mac, with Docker running (for the cross build).

## Phase 0 — spike (do this before installing)

These four unknowns must be confirmed on *your* device and OS version. Don't skip them —
they're the difference between a clean install and a soft-brick or a black screen.

1. **Config key names.** SSH in and inspect `~/.config/remarkable/xochitl.conf`. Confirm
   which key holds the passcode/PIN and which holds the cloud/Connect token:
   ```
   cat /home/root/.config/remarkable/xochitl.conf
   ```
   You don't need to edit anything by key — the design swaps whole directories — but you
   do need to know the passcode key to disable the native lock (step 5 below).

2. **Swap works.** Prove the core mechanic before trusting it:
   ```
   systemctl stop xochitl
   cp -a /home/root/.local/share/remarkable/xochitl /home/root/xochitl.bak
   # ...move it aside, start xochitl, confirm an empty library, then restore...
   systemctl start xochitl
   ```

3. **rm2fb.** The rM2 has no kernel framebuffer; a custom UI needs the
   [`remarkable2-framebuffer`](https://github.com/ddvk/remarkable2-framebuffer) shim.
   Install its server + client **standalone** (no Toltec). Note the path to
   `librm2fb_client.so` — it goes in the systemd unit's `LD_PRELOAD`. Then decide the
   boot model (next section).

4. **Auto-updates.** Find how to pause OTA updates on your OS version (commonly by
   pointing the update server to an unreachable host in
   `/usr/share/remarkable/update.conf`). Updates replace the rootfs and undo the `/etc`
   parts of this install — pause them, and run `reapply.sh` after any deliberate update.

## Boot model — pick one (Phase 0 decision)

The rm2fb server is normally the running xochitl process, which forces a choice about
*when* the pad draws. Both are documented in `systemd/rm-profile-pad.service`.

- **Model A — overlay (safer default).** xochitl starts normally; the pad runs *after* it
  as a standard rm2fb client and draws a fullscreen overlay. Proven path. Downside: a
  brief flash of xochitl content before the overlay paints.
- **Model B — pre-xochitl (flash-free).** A standalone rm2fb server starts first, the pad
  draws, then xochitl starts. No flash, but depends on a standalone server working before
  xochitl. Requires `systemctl disable xochitl` so only the pad starts it.

Start with Model A. Move to Model B only if the content flash bothers you and you've
confirmed a standalone rm2fb server works.

## Build the pad

The rM2 is ARMv7. Cross-compile with the same native-arm64 Docker path used in CI-less
local builds (avoids the `cross` bug on Apple Silicon):

```
docker run --rm -v "$PWD/pad":/work -w /work rust:bookworm bash -c 'set -e; apt-get update -qq; apt-get install -y -qq gcc-arm-linux-gnueabihf >/dev/null 2>&1; rustup target add armv7-unknown-linux-gnueabihf; export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc; cargo build --release --target armv7-unknown-linux-gnueabihf -p rm-profile-pad'
```

The binary lands at `pad/target/armv7-unknown-linux-gnueabihf/release/rm-profile-pad`.

## Copy the repo + binary to the device

```
scp -r bin systemd scripts docs root@10.11.99.1:/home/root/remarkable-profiles/
scp pad/target/armv7-unknown-linux-gnueabihf/release/rm-profile-pad root@10.11.99.1:/home/root/remarkable-profiles/
```

## Run setup

On the device:

```
cd /home/root/remarkable-profiles
sh scripts/rm-profile-setup --dry-run     # read every action first
sh scripts/rm-profile-setup               # then for real; it will prompt for PINs
```

It backs up, migrates your data into the `duncan` profile, creates the local-only `kid`
profile, installs the engine + pad + gate, and sets both PINs.

## Disable the native lock (avoid a double prompt)

Since the pad is the gate, turn off xochitl's own passcode in **both** profiles so users
aren't asked twice. Using the passcode key you found in Phase 0, clear it in each
profile's `config/xochitl.conf` (`profiles/duncan/config/…` and `profiles/kid/config/…`).
If you'd rather keep the native lock as a privacy backstop, expect a second prompt.

## First boot

```
systemctl reboot
```

Expect the PIN pad on boot. Type the owner PIN → owner notebooks. Power-cycle, type the
child PIN → child's empty local library. Verify:

```
/home/root/profiles/rm-profile status
/home/root/profiles/rm-profile list
```

## Day-to-day switching (Phase 1)

Switching users is a deliberate act: power the tablet off and on, then type the other
person's PIN. (Within one person's session, sleep/wake stays in their profile.) Seamless
switch-on-every-wake is Phase 2 and not part of this install.

## After an OS update

```
cd /home/root/remarkable-profiles
sh scripts/reapply.sh
systemctl reboot
```

## Troubleshooting

- **Pad shows nothing / black screen:** rm2fb isn't running or the `LD_PRELOAD` path in
  the unit is wrong. Recheck Phase 0 step 3 and the boot model.
- **`GLIBC_2.xx not found` when the pad runs:** the build toolchain's glibc is newer than
  your OS's. Rebuild in an older base image (`rust:bullseye`), or switch the target to
  `armv7-unknown-linux-musleabihf` for a static binary.
- **Asked for a PIN twice:** native lock still enabled — see "Disable the native lock".
- **Wrong profile after switch:** check `rm-profile status` and that `profiles/active`
  points where you expect.
- **Recover:** everything is under `/home/root`; restore `rmprofile-backup/` to undo.
