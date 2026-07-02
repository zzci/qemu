# Windows 11 guest (`OS=win11`)

**English** · [中文](./windows.zh-CN.md)

Run **Windows 11 Enterprise LTSC** on the `zzci/qemu` engine: a fully unattended install
(no clicks), virtio drivers slipstreamed, TPM 2.0 + UEFI, then boot straight from disk.

> **Why LTSC?** Windows 11 IoT Enterprise LTSC ships without Store / Copilot / consumer apps /
> Teams / widgets — already close to a "tiny11" image, more stable, and longer-serviced.

See the [root README](../../README.md) for the engine overview and quick start. This page is the
in-depth Windows guide.

---

## How it works

Two scripts own the Windows lifecycle (sharing the `qemu-common.sh` helpers):

| Script | Phase | Device model |
|--------|-------|--------------|
| `win11-installer` | one-shot install | Lean & disposable: OVMF + virtio-blk + AHCI(CD) + rng + plain VGA, **no TPM/NIC/balloon**, `cache=unsafe` for speed. Repacks your ISO (slipstream virtio + `autounattend.xml`) in `/tmp/win11-build`, installs into `windows.qcow2`, records the result, powers off. |
| `start-win11` | every boot | The **full** runtime model from a generated `-readconfig` file: TPM 2.0, virtio-blk/net, balloon, rng, USB (xhci + tablet), display. |

`start-win11` renders the static machine topology into `/storage/win11/windows.qemu.conf` and
launches QEMU with `-readconfig`; only dynamic / non-config-group args (`-cpu`, display, monitor,
identity, networking, USB, the data disk) stay on the command line.

The **TPM 2.0** itself is `swtpm`, run as the separate **`tpm` service** (not by `start-win11`), so
it is independently controllable (`sctl start|stop|restart tpm`) and reusable by other guests.
`start-win11` waits for the vTPM socket before booting and fails fast if it never comes up — see
[common/engine.md → vTPM](../common/engine.md#vtpm-the-tpm-service). The install runs **without** a
TPM (the `autounattend.xml` bypasses the check); the vTPM only attaches at runtime.

---

## Provide the ISO

The Windows ISO is **never auto-downloaded** (Microsoft eval URLs expire). Mount your own at
`/images/win11.iso`, or point `SOURCE_ISO` at any path (`-e SOURCE_ISO=/path/to.iso`):

```yaml
volumes:
  - ./images/Win11_LTSC_zh-cn.iso:/images/win11.iso:ro   # your ISO, surfaced as win11.iso
  - ./images/virtio-win.iso:/images/virtio-win.iso:ro    # virtio drivers (else auto-downloaded)
```

Before mastering, the installer inspects the ISO with `wiminfo`:

- an out-of-range `IMAGE_INDEX` is **fatal** (it prints the valid `1..N` range);
- if your `LANGUAGE` is not present in the image it **warns and falls back** to the image's
  default language, so the install still completes instead of stalling on the language screen.

---

## Install policy & state — `WIN11_INSTALL`

| Value | Behavior |
|-------|----------|
| `auto` | Install unless a completed install is recorded; also recovers an interrupted one. |
| `force` | Wipe + reinstall **once** (marker-guarded: a guest restart never re-wipes). |
| `none` (default) | Never auto-install; without a completed install, wait for a manual `win11-installer`. |

The decision is gated by an **install-state file**, `/storage/win11/windows.install`, whose first
line is `installing` or `installed`:

- `win11-installer` writes `installing` when it starts and `installed` only when the install
  finishes — so a half-written disk is **not** mistaken for a working one.
- `auto` installs whenever the status is not `installed` (fresh **or** interrupted); the boot guard
  refuses to boot until the status is `installed`.
- A disk that predates this file (no record) is **migrated** to `installed` on first contact, so
  existing installs keep booting.
- `force` reinstalls once and sets `windows.force-applied`; remove that marker (or run
  `FORCE=1 win11-installer`) to force again.

```bash
docker exec <container> win11-installer        # manual install, skips if recorded installed
FORCE=1 docker exec <container> win11-installer # rebuild from scratch
```

---

## Locale & edition

`LANGUAGE` / `REGION` / `KEYBOARD` take a friendly name or an `xx-XX` code; all default to `en-US`.

```yaml
environment:
  LANGUAGE: "Chinese"   # or zh-CN
  REGION: "zh-CN"
  KEYBOARD: "zh-CN"
```

> The chosen `LANGUAGE` must exist in the source ISO. An evaluation ISO is usually **en-US only**;
> use a multi-language LTSC ISO for other languages.

`IMAGE_INDEX` (default `1`) selects the `install.wim` edition; `1` is LTSC on the eval ISO.

---

## Account & RDP

`USERNAME` (default `docker`) / `PASSWORD` (default `admin`) are baked at install and are also the
RDP credentials. In the default `user` network mode, RDP is forwarded by `PORT_FWD` (default
`3389-3389`, i.e. host `3389` → guest `3389`); in `bridge`/`macvlan` the guest has its own LAN IP —
RDP straight to it. See [networking-and-devices.md](../common/networking-and-devices.md).

---

## Display: `std` to install, `virtio` to run

The installer always uses **plain VGA (`std`)** — the simplest, always-works adapter for Windows
Setup. The virtio-GPU driver (`viogpudo`) is slipstreamed during setup, so **after install you can
switch the runtime display to `virtio`** for widescreen resolutions and noVNC auto-resize.

```yaml
environment:
  VGA: "virtio"            # std | virtio | qxl
  RESOLUTION: "1920x1080"  # forced via EDID; best with VGA=virtio
```

- `std` boots at 1024×768 with a few 4:3 modes only.
- `virtio` + `RESOLUTION` gives a forced widescreen mode (applied a few seconds after the desktop
  loads on an existing install).
- The console may show **black** after the desktop loads — that is display sleep, not a hang; a
  keypress wakes it (RDP/`3389` reachable confirms the guest is up).

> An externally-built disk **without** the virtio-GPU driver can hang at the Windows Boot Manager
> under `VGA=virtio`. Install with this engine (driver slipstreamed) or revert to `VGA=std`.

---

## Storage & the per-VM config

All Windows state lives under `storage/win11/` (one dir per guest); the repacked install ISO is a
temporary artifact built in `/tmp/win11-build`, never in `/storage`.

```
storage/win11/
├── windows.qcow2          # the OS disk
├── windows.conf           # editable per-VM config (authoritative after first boot)
├── windows.qemu.conf      # generated -readconfig topology (do not edit; regenerated each boot)
├── windows.install        # install-state record (installing | installed)
├── windows.OVMF_VARS.fd   # UEFI NVRAM
└── windows.tpm/           # swtpm state
```

On first boot `start-win11` seeds `windows.conf` from the environment, then treats it as the source
of truth — **edit it and restart** to change resources/networking/display:

```ini
# /storage/win11/windows.conf
NAME=windows            # QEMU VM name (-name)
UUID=…                  # SMBIOS system UUID (-uuid), auto-generated once, then stable
CPU_CORES=4
RAM_SIZE=8G
MACHINE=q35
DISK=/storage/win11/windows.qcow2
NETWORK=user
PORT_FWD=3389-3389      # user-mode host-guest forwards (e.g. 3389-3389,8080-80)
VGA=virtio
RESOLUTION=1920x1080
USB=
SERIAL=                 # host serial -> guest COM, TTY paths (e.g. /dev/ttyUSB0,/dev/ttyS0)
EXTRA_ARGS=
```

A stable `UUID` is generated once and kept in the conf, so the guest's hardware identity
(licensing/activation) is consistent across restarts; each clone gets its own. To re-seed the conf
from the environment, delete the file and restart.

---

## Clones

```bash
docker exec <container> win11-clone dev          # linked clone (copy-on-write over a sealed base)
docker exec <container> win11-clone dev --full   # independent full copy
```

Each disk derives its own firmware/TPM/monitor/config/install-state from the disk path, so clones
run side by side off one `/storage`. Linked clones share a read-only `storage/win11/base.qcow2`.
Clones inherit the source's machine SID/hostname — run `sysprep /generalize` inside Windows before
cloning if you need unique identities.

```bash
docker run -d --name win11-dev --device=/dev/kvm \
  -e ZSRV_vm=true -e ZSRV_tpm=true -e ZSRV_novnc=true \
  -e WIN11_INSTALL=none -e DISK=/storage/win11/clones/dev.qcow2 \
  -p 8016:8006 -v "$PWD/storage:/storage" zzci/qemu
```

---

## Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| Install loops to "The computer restarted unexpectedly" | invalid `autounattend.xml`; read `/var/log/supervisord-vm.log`, or mount the qcow2 and read `C:\Windows\Panther\setuperr.log` |
| Install stalls on the language screen | `LANGUAGE` not in the ISO — the installer warns and falls back to the ISO default; use a multi-language LTSC ISO |
| Container waits, "no completed install" | status is not `installed` (fresh or interrupted) — run `win11-installer`, or set `WIN11_INSTALL=auto` |
| Boot hangs at "Windows Boot Manager" after switching `VGA` | the disk lacks the new display driver — revert to `VGA=std`, or reinstall with the target adapter |
| Console black but RDP works | display sleep, not a hang — press a key to wake |
| `/dev/kvm` warning / very slow | started without `--device=/dev/kvm` → TCG; add the device |
