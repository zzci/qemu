# qemu — QEMU/KVM guests in Docker

**English** · [中文](./README.zh-CN.md)

A small **QEMU/KVM virtualization engine** packaged as one Docker image (`zzci/qemu`) and operated
from your browser. The engine is **OS-agnostic**: pick a guest with `OS=` and `start-vm` dispatches
to that guest's starter. Built on [`zzci/ubase`](https://hub.docker.com/r/zzci/ubase) (Ubuntu 22.04
+ tini + supervisord). Everything — preparing media, installing, booting, cloning — happens
**inside the container**, configured by environment variables and an editable, incus-style per-VM
config.

Engine features: browser console (noVNC) · KVM acceleration · pluggable networking
(NAT / bridge / macvlan / host) · USB & serial passthrough · per-guest persistent storage ·
clone VMs without reinstalling.

📚 **Guides:** [engine](./docs/common/engine.md) ·
[networking & devices](./docs/common/networking-and-devices.md) · [docs/](./docs/) ·
[Contributing / add a guest](./docs/CONTRIBUTING.md)

---

## Supported systems

Select a guest with `OS=`. Each guest has its own guide for install media, knobs and quirks.

| `OS=` | Guest | Firmware | Install | Status | Guide |
|-------|-------|----------|---------|--------|-------|
| `win11` (default) | Windows 11 Enterprise **LTSC 2024** | UEFI + TPM 2.0 | unattended (you provide an ISO) | stable | [windows.md](./docs/guests/windows.md) |
| `alpine` | Alpine Linux | SeaBIOS | console install over noVNC | stable | [alpine.md](./docs/guests/alpine.md) |

**Add your own:** drop a `start-<os>` script in `rootfs/build/bin/`, wire a case into `start-vm`,
and add `docs/guests/<os>.md` — see [common/engine.md → Adding a guest](./docs/common/engine.md#adding-a-guest)
and [CONTRIBUTING.md](./docs/CONTRIBUTING.md).

---

## Contents

- [Architecture](#architecture)
- [Requirements](#requirements)
- [Quick start](#quick-start)
- [Configuration](#configuration)
- [Networking](#networking)
- [USB passthrough](#usb-passthrough)
- [Logs](#logs)
- [Access](#access)
- [File layout](#file-layout)
- [Troubleshooting](#troubleshooting)
- [Notes & limitations](#notes--limitations)

---

## Architecture

The container runs a **single service, `vmd`** (a small Rust binary, toggled by `ZSRV_vmd`). vmd owns
the QEMU process, swtpm (vTPM), and the web console + power API on port 8006 — it replaces the old
per-service bash (`websockify`/`socat` included). It contains **no per-OS logic**: each guest declares
its QEMU command, sidecars, install command and setup as **data** in `vmd.toml`, so adding a guest is
a config edit, never a code change. Deep dive: [supervisor/README.md](./supervisor/README.md).

---

## Requirements

- Docker with access to the host **`/dev/kvm`** (`--device=/dev/kvm`); without it QEMU falls back
  to TCG software emulation, far too slow for a real install.
- Install media **per guest** — e.g. a Windows 11 LTSC ISO you provide (see the guest's guide).
- For bridge/macvlan networking: `--cap-add NET_ADMIN` and `--device=/dev/net/tun`.

---

## Quick start

Guests and all their parameters live in **`vmd.toml`** (see [supervisor/vmd.toml](./supervisor/vmd.toml));
the default guest is Windows 11. Full Windows details — ISO, install policy, locale, display — are in
[docs/guests/windows.md](./docs/guests/windows.md).

```bash
cd qemu
mkdir -p images storage
cp /path/to/Win11_LTSC.iso images/win11.iso       # REQUIRED for win11 — provide your own ISO

docker build -t zzci/qemu .

docker run -d --name win11 --device=/dev/kvm \
  -e ZSRV_vmd=true -e VMD_OS=win11 \
  -p 8006:8006 -p 3389:3389 \
  --stop-timeout=180 \
  -v "$PWD/storage:/storage" -v "$PWD/images:/images:ro" \
  -v "$PWD/supervisor/vmd.toml:/storage/vmd.toml:ro" \
  zzci/qemu

docker logs -f win11                              # watch install/boot
# then open  http://localhost:8006/   (web console: VNC + serial + power controls)
```

> The single `vmd` service is **off until enabled** — pass `-e ZSRV_vmd=true`. Pick the guest with
> `-e VMD_OS=<name>`; everything else (RAM, disk, install policy, the QEMU command…) is in
> `vmd.toml`. On `docker stop`, vmd asks the guest to ACPI power off and waits, so give it headroom
> (`--stop-timeout=180`, or compose `stop_grace_period: 3m`); a guest that powers off from inside
> **stays off** (`docker restart` to boot again). See `vmd print` to inspect the exact QEMU command.

For other guests, set `OS=` and follow that guest's guide (e.g.
[Alpine](./docs/guests/alpine.md)). A `docker-compose.yml` is included with a Windows service and a
commented Alpine example.

---

## Configuration

These **engine** knobs apply to any guest, set at `docker run -e ...` / compose `environment:`:

| Variable | Default | Description |
|----------|---------|-------------|
| `OS` | `win11` | Guest selector — see [Supported systems](#supported-systems) |
| `RAM_SIZE` | `4G` | Guest memory |
| `CPU_CORES` | `2` | vCPUs |
| `DISK_SIZE` | `128G` | System disk size (first install only) |
| `NETWORK` | `user` | `user` \| `bridge` \| `macvlan` \| `host` \| `none` |
| `BRIDGE` / `MACVLAN` | – | Bridge name / container iface(s) for those modes |
| `PORT_FWD` | per-guest | `user`-mode host→guest forwards, `host-guest` pairs (e.g. `3389-3389,8080-80`) |
| `VGA` | `std` | Display adapter: `std` \| `virtio` \| `qxl` |
| `RESOLUTION` | – | Force a resolution, e.g. `1920x1080` (EDID) |
| `VNC_HOST` | `127.0.0.1` | VNC bind address. Localhost = console only via the noVNC bridge; set `0.0.0.0` to expose VNC directly (e.g. under `--network host`) |
| `VNC_PASSWORD` | – | VNC password (empty = no auth). The VNC protocol truncates it to 8 characters |
| `USB` | – | USB passthrough, `vendor:product` hex (comma list) |
| `SERIAL` | – | Host serial → guest COM, TTY path(s) (e.g. `/dev/ttyUSB0`), comma list |
| `CONSOLE` | `off` | `on` exposes the guest text console (ttyS0/COM1) for `vm-console` — useful for Linux |
| `EXTRA_ARGS` | – | Raw extra QEMU args (socket serial, extra NIC/disk, vfio-pci…) |
| `DISK` | `/storage/<os>/…qcow2` | Boot disk path (point at a clone) |
| `NAME` / `UUID` | `windows` / auto | QEMU `-name` / SMBIOS `-uuid` (UUID generated once, kept in the conf) |
| `ZSRV_vm` | off | set `true` to boot the guest; leave off for build-only (then `win11-installer`) |
| `ZSRV_tpm` | off | set `true` to run the vTPM (`tpm` service) — required for Windows 11 |
| `ZSRV_novnc` | off | set `true` to run the browser console bridge |

**Guest-specific knobs** (accounts, locale, install policy, port forwards…) are documented in each
guest's guide: [Windows 11](./docs/guests/windows.md), [Alpine](./docs/guests/alpine.md). On first
boot the chosen settings are seeded into an editable per-VM config under `storage/<os>/` — see
[common/engine.md](./docs/common/engine.md).

---

## Networking

Select the mode with `NETWORK`; each guest NIC is one adapter in the guest. Full guide (incl. USB &
serial passthrough): [docs/common/networking-and-devices.md](./docs/common/networking-and-devices.md).

| Mode | Behavior | Requires |
|------|----------|----------|
| `user` | SLIRP NAT + port forwards (default) | nothing |
| `bridge` | tap on an existing bridge `$BRIDGE`; guest joins that L2 | `NET_ADMIN`, `/dev/net/tun`, the bridge |
| `host` | same as bridge, for `--network host` | as bridge |
| `macvlan` | macvtap on `$MACVLAN`; guest gets its own LAN IP | `NET_ADMIN`, `--device-cgroup-rule='c *:* rwm'`, a macvlan net |
| `none` | no NIC | – |

```bash
docker network create -d macvlan \
  --subnet=192.168.1.0/24 --gateway=192.168.1.1 -o parent=enp1s0 lan
docker run -d --name win11 --network lan --device=/dev/kvm --device=/dev/net/tun \
  --cap-add NET_ADMIN --device-cgroup-rule='c *:* rwm' \
  -e NETWORK=macvlan -e MACVLAN=eth0 -v "$PWD/storage:/storage" zzci/qemu
```

> macvlan caveat: the **host** cannot reach a macvlan guest IP directly (other LAN machines can).

---

## USB passthrough

Pass host USB devices by `vendor:product` (hex), comma-separated. Details (incl. serial ports) in
[docs/common/networking-and-devices.md](./docs/common/networking-and-devices.md).

```bash
docker run -d --name win11 --device=/dev/kvm --device=/dev/bus/usb \
  -e USB=0bda:8153 -v "$PWD/storage:/storage" zzci/qemu
```

The host USB device must be visible in the container (`--device=/dev/bus/usb`, or privileged).

---

## Logs

supervisord writes per-service logs **inside the container** (not on the `/storage` volume):

```
/var/log/supervisord-vm.log      # QEMU / install / boot
/var/log/supervisord-tpm.log     # vTPM
/var/log/supervisord-novnc.log   # console bridge
```

```bash
docker exec <container> tail -f /var/log/supervisord-vm.log
```

---

## Access

- **Graphical console**: `http://localhost:8006/` — full mouse/keyboard noVNC.
- **Text console** (serial, great for Linux): boot with `CONSOLE=on`, then attach a terminal —
  `docker exec -it <container> vm-console` (detach with Ctrl-]). The Linux guest needs
  `console=ttyS0` for a login/boot console there.
- **Remote**: per guest — Windows RDP (guest `3389`), Alpine SSH (guest `22`). In `user` mode the
  host↔guest mapping is set by `PORT_FWD` (default `3389-3389` / `2222-22`).

---

## File layout

```
qemu/
├── Dockerfile                 # FROM zzci/ubase + qemu/ovmf/swtpm/novnc + tools
├── docker-compose.yml         # project name "qemu" (windows service + commented alpine example)
├── README.md / README.zh-CN.md
├── docs/                      # bilingual guides (each EN + .zh-CN) + CONTRIBUTING.md
│   ├── common/                # OS-agnostic: engine.md, networking-and-devices.md
│   └── guests/                # per-OS: windows.md, alpine.md, _template.md
├── images/                    # local install ISOs — gitignored
├── storage/                   # persistent state, one dir per guest (logs live in /var/log)
│   ├── win11/                 # windows.qcow2, *.conf / *.qemu.conf / *.install, OVMF_VARS, *.tpm/, clones/
│   └── alpine/                # alpine.qcow2 (when OS=alpine)
└── rootfs/build/{bin,services,config}   # baked into the image
```

---

## Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| Nothing boots on a bare `docker run` | services are off until enabled — add `-e ZSRV_vm=true -e ZSRV_tpm=true -e ZSRV_novnc=true` |
| `vm` service flapping in `sctl list` | check `/var/log/supervisord-vm.log`; usually a QEMU arg or a stale qcow2 lock from a previous container |
| Console blank / `/dev/kvm` warning | started without `--device=/dev/kvm` → TCG; add the device |
| `macvtap … Operation not permitted` | add `--device-cgroup-rule='c *:* rwm'` (and `--cap-add NET_ADMIN`) |
| `NETWORK=bridge needs BRIDGE=…` | set `BRIDGE` to an existing bridge and add `--device=/dev/net/tun` |

Guest-specific issues (install loops, language, display) are in each guest's guide.

---

## Notes & limitations

- The OS lives in the mounted `storage/<os>/…qcow2`, not baked into the image — the image itself is
  small (~750 MB).
- Install media is per guest and user-provided where licensing requires (e.g. a Windows ISO is
  never auto-downloaded).
- For virtualization/lab use; respect each OS's licensing terms.
