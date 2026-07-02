# Alpine Linux guest (`VMD_OS=alpine`)

**English** · [中文](./alpine.zh-CN.md)

A minimal Linux guest that installs itself with **zero media**: the install script downloads the
official Alpine cloud image (tiny-cloud, BIOS/SeaBIOS) and turns it into the guest disk. Boots to a
serial console in seconds.

## Configuration

```toml
[guest.alpine]
dir       = "/vms/alpine"
disk      = "alpine.qcow2"
disk_size = "8G"                # the cloud image is resized to this
ram       = "2G"
cpus      = 2
launch    = "alpine"

[guest.alpine.install]
policy = "auto"
# url          = "https://…/custom.qcow2"   # explicit image (skips mirror resolution)
# alpine_mirror = "https://mirror…/alpine/latest-stable/releases/cloud"
# sha512       = "<hex>"                    # pin the image checksum (optional)
```

The installer resolves the newest `generic_alpine-*-x86_64-bios-tiny-r*.qcow2` from the mirror
index, `qemu-img convert`s it to `VMD_DISK` and resizes to `VMD_DISK_SIZE`. The download is
verified against a `sha512` pin, or else the mirror's published `<file>.sha512` (a mismatch
aborts; only a missing checksum is just warned about). `FORCE=1` (`policy = "force"`)
re-downloads and rebuilds.

## Access

- **Serial console** — the primary interface: open the web console → 打开控制台 / Open Console
  (`/console`, COM1 with `console=ttyS0`).
- **VNC** — a plain VGA text screen is also available.
- **SSH** — the template forwards host 2222 → guest 22 (`docker run -p 127.0.0.1:2222:2222`);
  enable sshd in the guest first.

The cloud image uses tiny-cloud: on first boot without a datasource it comes up with a `root`
account without a password on the serial console (set one immediately, or grow the setup with
cloud-init user data — mount a nocloud seed ISO via an extra CD-ROM drive).

## Notes

- SeaBIOS, no TPM/UEFI — the launcher is the simplest possible template and a good starting point
  for new guests.
- Resize later: `qemu-img resize {dir}/alpine.qcow2 +8G`, then grow the partition inside the
  guest.
