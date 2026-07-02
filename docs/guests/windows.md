# Windows 11 guest (`VMD_OS=win11`)

**English** · [中文](./windows.zh-CN.md)

Fully unattended **Windows 11** install and operation: provide an ISO, first boot installs
(~13 min with KVM), every later boot goes straight to Windows. UEFI (OVMF) + vTPM 2.0 + virtio
disk/net (drivers slipstreamed), RDP enabled, deterministic ACPI shutdown.

## Requirements

- `images/win11.iso` mounted at **`/images/win11.iso`** — a Windows 11 ISO you provide (LTSC
  recommended; never auto-downloaded).
- `/images/virtio-win.iso` optional — downloaded from Fedora automatically if absent.
- `--device=/dev/kvm`.

## Configuration

```toml
[guest.win11]
dir       = "/vms/win11"
disk      = "windows.qcow2"
disk_size = "128G"              # sparse; grows on demand
ram       = "4G"
cpus      = 2
launch    = "win11"
tpm       = true
seed      = [ { template = "/usr/share/OVMF/OVMF_VARS_4M.fd", to = "{state}.OVMF_VARS.fd" } ]

[guest.win11.install]
policy      = "auto"            # auto | force | none
source_iso  = "/images/win11.iso"
virtio_iso  = "/images/virtio-win.iso"
username    = "docker"          # local admin created by the unattend
password    = "admin"
language    = "en-US"           # UI language; must exist in the ISO (validated, auto-falls back)
region      = "en-US"           # user locale (optional; defaults to language)
keyboard    = "en-US"           # input locale (optional; defaults to language)
image_index = 1                 # install.wim edition index (1 = LTSC on the LTSC ISO)
# virtio_sha256 = "<hex>"       # verify a downloaded virtio-win.iso (unverified when unset)
```

Install keys become UPPERCASE env vars for `{dir}/scripts/install`; `language` accepts friendly
names too (`Chinese` → `zh-CN`). A zh-CN system: set `language/region/keyboard = "zh-CN"` and use
a zh-CN ISO.

## What the installer does

`{dir}/scripts/install` (your editable copy) owns the whole pipeline:

1. extract the source ISO; validate `image_index` and `language` against `install.wim`
   (an unavailable language falls back to the image default instead of hanging setup);
2. render `autounattend.xml` (accounts, locale, TPM/SecureBoot/RAM checks bypassed, RDP on,
   telemetry trimmed, **hibernation off + power button = shutdown** for reliable ACPI control);
3. slipstream virtio drivers (`$WinpeDriver$`); remaster a no-prompt UEFI ISO (boots without
   "Press any key");
4. run a lean throwaway install VM (`cache=unsafe`, no TPM/NIC) and wait for its clean power-off;
5. write `{state}.install` = `installed`; vmd then boots the real device model.

Signals while installing: watch it live on the web console; the qcow2 grows; the vmd log prints
`install finished in Ns`.

## Access

- **Web console** — `http://<host>:8006` (noVNC display + power controls).
- **RDP** — port 3389 is forwarded by the launcher and enabled by the unattend
  (`docker run -p 127.0.0.1:3389:3389`, log in as `username`/`password`).

## Cloning

```bash
docker exec <container> win11-clone <name>          # linked clone (copy-on-write, instant)
docker exec <container> win11-clone <name> --full   # full standalone copy
```

Then add a `[guest.<name>]` block pointing `disk` at the clone and run it (own container with
`VMD_OS=<name>`, or switch the current one). Clones share the source's SID/hostname — fine for
labs.

## Troubleshooting

- **Setup waits on a language screen** — ISO doesn't contain the configured `language`; the
  validator normally auto-falls back (see the install log).
- **Reinstall** — `policy = "force"` (one-shot wipe) or delete `{state}.install*` + the disk.
- **Display resolution** — edit `xres/yres` in `{dir}/scripts/launcher` (default 1920×1080).
- **Slow install** — check the log for a TCG warning; you need working `/dev/kvm`.
