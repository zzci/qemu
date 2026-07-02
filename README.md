# qemu — QEMU/KVM guests in Docker

**English** · [中文](./README.zh-CN.md)

A small **QEMU/KVM virtualization engine** packaged as one Docker image (`zzci/qemu`) and operated
from your browser. The engine is a single static Rust binary, **`vmd`**, that supervises QEMU over
QMP; it contains **no per-OS logic**. Guests are pure configuration: a `[guest.<name>]` block in
`vmd.toml` plus a template folder of scripts. Built on
[`zzci/ubase`](https://hub.docker.com/r/zzci/ubase) (Ubuntu 22.04 + tini + supervisord).

Features: embedded web console (noVNC + serial terminal, English/中文) · KVM acceleration ·
unattended installs (Windows 11, Alpine) · vTPM 2.0 · VNC & serial over unix sockets (no open TCP
except the web port) · power API with deterministic ACPI shutdown · optional access password ·
per-guest persistent home with user-editable scripts.

📚 **Guides:** [engine](./docs/common/engine.md) ·
[networking, serial, USB & devices](./docs/common/networking-and-devices.md) ·
[Windows](./docs/guests/windows.md) · [Alpine](./docs/guests/alpine.md) ·
[Contributing / add a guest](./docs/CONTRIBUTING.md)

---

## Requirements

- Docker with access to the host **`/dev/kvm`** (`--device=/dev/kvm`); without it QEMU falls back
  to TCG software emulation, far too slow for real use.
- Install media where a guest needs it — e.g. a Windows 11 ISO you provide. Alpine installs itself
  from the official cloud image (network access only).
- For bridge/tap networking: `--cap-add NET_ADMIN` and `--device=/dev/net/tun`.

## Quick start

```bash
docker run -d --name qemu --device=/dev/kvm \
  -e ZSRV_vmd=true -e VMD_OS=win11 \
  -v "$PWD/vms:/vms" -v "$PWD/images:/images:ro" \
  -p 127.0.0.1:8006:8006 -p 127.0.0.1:3389:3389 zzci/qemu
```

- Put your Windows ISO at `images/win11.iso` (and optionally `images/virtio-win.iso`; it is
  downloaded otherwise). First boot runs a fully unattended install (~13 min with KVM), then boots
  the system. Open **http://localhost:8006** to watch and control it.
- `VMD_OS=alpine` instead boots an Alpine guest — no media needed, the installer fetches the
  official cloud image.

Or use [docker-compose.yml](./docker-compose.yml).

## Security

The defaults are tuned for a single-host lab, so review them before exposing anything:

- **Bind to localhost (the default above) until auth is on.** Without `[web] password` the console
  gives full VNC + power control to anyone who can reach port 8006, and the sample guest account
  (`docker`/`admin`) is public knowledge — change it, and keep 3389 private or firewalled.
- **Put TLS in front for remote access.** vmd serves plain HTTP; run it behind an HTTPS reverse
  proxy (the login cookie is marked `Secure` automatically when `X-Forwarded-Proto: https` is set).
  Repeated wrong passwords are tarpitted, but transport privacy is the proxy's job.
- **Pin download checksums.** The Alpine installer verifies the cloud image against the mirror's
  `.sha512` (or a `sha512 = "…"` install key); the Windows installer verifies a downloaded
  `virtio-win.iso` only when `virtio_sha256 = "…"` is set.

## Configuration

Everything lives in **`vmd.toml`**. Search order: `$VMD_CONFIG` → `/vms/vmd.toml` →
`/etc/vmd/vmd.toml` (baked default, auto-copied to `/vms/vmd.toml` on first run so you can edit it).
Select the active guest with `VMD_OS` or the file's `default`.

```toml
default = "win11"

[web]
port = 8006
# password = "change-me"        # web console access password (empty/absent = no auth)

[guest.win11]
dir       = "/vms/win11"        # guest home: disk, state, logs and scripts/ live here
disk      = "windows.qcow2"     # relative to dir
disk_size = "128G"
ram       = "4G"
cpus      = 2
launch    = "win11"             # template folder, copied to {dir}/scripts/ (edit those copies)
tpm       = true                # built-in vTPM 2.0 (managed swtpm)
seed      = [ { template = "/usr/share/OVMF/OVMF_VARS_4M.fd", to = "{state}.OVMF_VARS.fd" } ]

[guest.win11.install]           # first-boot install, gated by policy
policy      = "auto"            # auto | force | none
source_iso  = "/images/win11.iso"
username    = "docker"
password    = "admin"
language    = "en-US"           # zh-CN, ja-JP, … (must exist in the ISO; auto-falls back)
image_index = 1
```

Every install key becomes an **UPPERCASE env var** for the install script; the guest's disk/size
arrive as `VMD_DISK`/`VMD_DISK_SIZE`. Details: [docs/common/engine.md](./docs/common/engine.md).

## The web console

One port (8006) serves everything: a home screen with live status, VM facts and port forwards;
**开机/关机/重启/强制关闭** power controls; the noVNC display; an xterm serial console; and a
JSON API (`/status`, `/info`, `POST /power/<start|shutdown|reset|poweroff>`). UI is bilingual
(English/中文, toggle in the header). Set `[web] password` to require a login first.

CLI equivalent inside the container:

```bash
vmd power status|start|shutdown|reset|poweroff
vmd print          # dry run: show the resolved plan + QEMU command
```

## Customizing a guest

On first boot the guest's template scripts are copied to `{dir}/scripts/` (e.g.
`vms/win11/scripts/launcher`, `.../install`). **Edit those copies** — they are yours and are never
overwritten. The launcher builds the QEMU command from `VMD_*` env vars; change resolution, add
disks, NICs, serial ports or USB devices there. See
[networking & devices](./docs/common/networking-and-devices.md).

## File layout (per guest, under `dir`)

```
vms/win11/
├── windows.qcow2            # the disk
├── windows.OVMF_VARS.fd     # UEFI NVRAM
├── windows.tpm/             # vTPM state
├── windows.{qmp,vnc,console}.sock
├── windows.install(ed)      # install markers
├── windows.uuid             # stable SMBIOS UUID
├── qemu.log                 # QEMU's own output
└── scripts/                 # your editable launcher + install
```

## Troubleshooting

- **Slow / TCG warning in the log** — the container has no usable `/dev/kvm`.
- **Install seems stuck** — watch it live over the web console; the installer powers the VM off
  when done. `vms/<g>/<disk>.install` holds `installing`/`installed`.
- **Reinstall from scratch** — set `policy = "force"` (one-shot wipe), or delete the disk and
  markers.
- **Logs** — `docker exec <c> tail -f /var/log/supervisord-vmd.log`; QEMU's own stderr is in
  `{dir}/qemu.log`.
