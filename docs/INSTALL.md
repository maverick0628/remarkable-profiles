# Install runbook — reMarkable 2

The code is built and tested off-device; these steps run against your actual tablet over
SSH.

> **reMarkable 2 only.** Not rM1, not Paper Pro.
>
> **Convenience and basic privacy, not security.** Anyone with USB/SSH access can read
> every profile.
>
> **Back up first, and copy the backup off the device.** Migration moves your real
> notebooks. The migrate script backs up too, but verify it and pull a copy to your
> computer before continuing.

Two models. Check your OS version first (**Settings → General → Help → About**, or
`cat /etc/os-release`):

- **OS newer than ~3.3 → engine + handoff (below).** The only working model on current
  firmware. This is what's deployed.
- **OS ≤ 3.3.2.1666 → optional auto-by-PIN pad.** See the appendix.

## 1. SSH access

Get the root password from **Settings → General → Help → Copyrights and licenses**
(bottom). USB IP is `10.11.99.1`. Install your key so commands run without a password:

```
ssh-copy-id -i ~/.ssh/id_ed25519.pub root@10.11.99.1
ssh root@10.11.99.1 echo ok
```

## 2. Deploy the engine + migrate script

```
ssh root@10.11.99.1 'mkdir -p /home/root/profiles /home/root/remarkable-profiles'
scp bin/rm-profile root@10.11.99.1:/home/root/profiles/rm-profile
scp -r bin scripts root@10.11.99.1:/home/root/remarkable-profiles/
ssh root@10.11.99.1 'chmod +x /home/root/profiles/rm-profile'
```

## 3. Migrate (creates User 1 + local-only User 2)

Dry-run first, then for real. It backs up and moves your data into the User 1 profile.
Then it creates a local-only User 2 profile seeded from a sanitized copy of your config.
Cloud tokens and passcode are stripped, so User 2 can't sync to your account.

```
ssh root@10.11.99.1 'sh /home/root/remarkable-profiles/scripts/rm-profile-migrate --dry-run'
ssh root@10.11.99.1 'sh /home/root/remarkable-profiles/scripts/rm-profile-migrate user1 user2'
```

Both names are optional and default to `user1` and `user2`. If you pick your own, use
lowercase letters, digits, `_` and `-`. Start with a letter or digit and keep it to 32
characters. The wifi key in step 5a only switches to names like that.

**Copy the backup off-device before confirming the prompt:**
`scp -r root@10.11.99.1:/home/root/rmprofile-backup ./`

The tablet comes back on the User 1 profile, unchanged (your PIN, notebooks, cloud sync).

## 4. Verify

```
ssh root@10.11.99.1 '/home/root/profiles/rm-profile list'           # * user1  /  user2
ssh root@10.11.99.1 '/home/root/profiles/rm-profile switch user2'   # User 2: empty local library
ssh root@10.11.99.1 '/home/root/profiles/rm-profile switch user1'   # back to your library
```

## 5. Handoff switch trigger

Find the tablet's wifi IP: `ssh root@10.11.99.1 'ip -4 addr show wlan0'`. Reserve the DHCP
lease or add a `.lan` record so it's stable.

### 5a. Enable SSH over wifi (required on OS 3.x+)

Newer reMarkable OS ships dropbear **socket-activated on USB only**
(`dropbear-usb0/usb1.socket`), so any LAN/phone SSH is *refused* until you flip
reMarkable's own wifi-SSH toggle — a marker file gated by
`dropbear-wlan.socket.d/override.conf`. Because our live config path is a symlink into the
*active* profile, create the marker in **every** profile so it stays on across switches:

```
ssh root@10.11.99.1 'touch /home/root/profiles/user1/config/rm_enable_ssh_wifi_marker /home/root/profiles/user2/config/rm_enable_ssh_wifi_marker; systemctl restart dropbear-wlan.socket'
```

Confirm: `systemctl show dropbear-wlan.socket -p ConditionResult` → `yes`. This lives under
`/home/root`, so it survives OS updates.

**This opens root SSH on your LAN.** Harden the wifi-exposed key so it can only switch
profiles: deploy `bin/rm-ssh-forced`, then prefix that key's line in
`/home/root/.ssh/authorized_keys` with
`command="/home/root/profiles/rm-ssh-forced",no-port-forwarding,no-agent-forwarding,no-pty`.
The wrapper only accepts the exact command `/home/root/profiles/rm-profile switch <name>`,
where `<name>` is an existing profile with a safe name. It denies everything else, including
extra arguments. There is no list of names to keep in sync. (Password auth over wifi
remains available; disable it separately if you want.)

**From a Mac:** `RM_HOST=<wifi-ip> scripts/rm-switch user2` (and `… user1`).

**iPhone Shortcut (handoff-friendly):**
1. Shortcuts → **+** → **Run Script Over SSH**. Host = wifi IP, Port 22, User `root`.
2. Auth **SSH Key** → **Share Public Key** → add it to the tablet:
   `printf '%s\n' '<paste key>' | ssh root@10.11.99.1 'umask 077; mkdir -p /home/root/.ssh; cat >> /home/root/.ssh/authorized_keys; chmod 600 /home/root/.ssh/authorized_keys'`
3. Script: `/home/root/profiles/rm-profile switch user2`. Name it. Duplicate for `user1`.
   Keep the script to that one line. The forced command denies anything extra.
4. Add each to the Home Screen for one-tap.

Caveats: the tablet must be **awake** (wifi drops in sleep), and switching takes ~3-5s
with a screen flash while xochitl restarts.

## Setting User 2's PIN

The User 2 profile ships with no PIN. Switch to it, then set one on-device via
**Settings → Security** if you want that side locked too. User 1's PIN is unchanged.

## Existing installs

Installs made with older default profile names keep working under those names. The
profile directories stay as they are and the new forced command accepts any existing
profile with a safe name. Your shortcuts keep working as long as they send the one-line
`rm-profile switch <name>` script from step 5. To upgrade, copy the new engine and forced
command over:

```
scp bin/rm-profile bin/rm-ssh-forced root@10.11.99.1:/home/root/profiles/
```

Renaming to `user1` and `user2` is optional. If you want the new names, replace `<old-1>`
and `<old-2>` with your current profile names and run these over USB. xochitl stops first
because its live paths run through `active`. The tablet comes back on User 1.

```
ssh root@10.11.99.1 'systemctl stop xochitl'
ssh root@10.11.99.1 'cd /home/root/profiles && mv <old-1> user1 && mv <old-2> user2'
ssh root@10.11.99.1 'ln -sfn user1 /home/root/profiles/active && systemctl start xochitl'
```

If you use the PIN pad, rename its records in `pins.conf` too:

```
ssh root@10.11.99.1 "sed -i -e 's/^<old-1>:/user1:/' -e 's/^<old-2>:/user2:/' /home/root/profiles/pins.conf"
```

Then update anything that switches by name. Change each iPhone Shortcut's script to
`/home/root/profiles/rm-profile switch user1` or `/home/root/profiles/rm-profile switch user2`,
and use the new names with `scripts/rm-switch`.

## Recovery

Everything is under `/home/root`. To undo: `rm-profile switch user1`, stop xochitl, and
restore `rmprofile-backup/` over `~/.local/share/remarkable` and `~/.config/remarkable`.

---

## Appendix — auto-by-PIN pad (OS ≤ 3.3 only)

On OS ≤ 3.3.2.1666, the custom PIN pad can draw via `rm2fb`, giving true auto-by-PIN
(type your PIN at boot, land in your profile). On newer OS it does not work: `rm2fb` has no
offsets past 3.3.2.1666 and the rM2's packed framebuffer can't be driven directly.

If you're on a supported OS: install
[`remarkable2-framebuffer`](https://github.com/ddvk/remarkable2-framebuffer) (note the
`librm2fb_client.so` path), cross-build the pad (see below), then run
`scripts/rm-profile-setup` and pick a boot model per the comments in
`systemd/rm-profile-pad.service`. Disable the native lock in both profile configs so the
pad is the sole gate.

Cross-build the pad (native arm64 Docker avoids the `cross` bug on Apple Silicon):

```
docker run --rm -v "$PWD/pad":/work -w /work rust:bookworm bash -c 'set -e; apt-get update -qq; apt-get install -y -qq gcc-arm-linux-gnueabihf >/dev/null 2>&1; rustup target add armv7-unknown-linux-gnueabihf; export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc; cargo build --release --target armv7-unknown-linux-gnueabihf -p rm-profile-pad'
```
