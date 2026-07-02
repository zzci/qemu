# The engine (OS-agnostic)

**English** · [中文](./engine.zh-CN.md)

`zzci/qemu` is one Docker image that runs many guests. This page covers the parts that are the
**same for every guest** — dispatch, services, the console, the per-VM config/state convention, the
storage layout, and **how to add a new guest**. Per-OS specifics live under
[../guests/](../guests/); networking and device passthrough are in
[networking-and-devices.md](./networking-and-devices.md). Overview: [root README](../../README.md).

---

## Dispatch — one image, many guests

The container's `CMD` (from `zzci/ubase`) is `/start.sh` → supervisord, which runs three services:

| Service | Script | Role |
|---------|--------|------|
| `vm` | `start-vm` | Reads `$OS` and `exec`s the matching starter (`win11`→`start-win11`, `alpine`→`start-alpine`). |
| `tpm` | `start-tpm` | Runs `swtpm` (vTPM 2.0) in the foreground for guests that need one; toggled by `ZSRV_tpm`. |
| `novnc` | `start-novnc` | `websockify` bridges the browser console on port `8006` to QEMU's VNC (`:0` / `5900`). |

Services are toggled with ubase `ZSRV_*` env vars (`ZSRV_vm`, `ZSRV_tpm`, `ZSRV_novnc`) and
controlled at runtime with `sctl start|stop|restart <name>`. **Nothing is baked into the image**, so
each service is **off until explicitly enabled** — a bare `docker run` boots nothing. Enable what you
need, e.g. `-e ZSRV_novnc=true -e ZSRV_tpm=true -e ZSRV_vm=true`. Leave `ZSRV_vm` off for a
build-only container (then `docker exec <c> win11-installer`).

`start-vm` stays small — it seeds the **OS-agnostic engine defaults** (RAM, CPUs, disk, machine,
network, display; all overridable with `docker run -e …`) and exports them, then dispatches.
Guest-specific config (e.g. the Windows account/locale/install policy) lives in that guest's own
starter, not here. Each guest's starter **owns its own install + boot logic** (so guests can't break
each other) and pulls only shared, generic helpers from **`qemu-common.sh`** (logging, KVM/firmware
detection, VNC, host-forward glue, graceful-shutdown supervision) — no guest logic is shared.

**VM lifecycle.** On `docker stop` the starter asks the guest to ACPI power off and **waits** for
QEMU to finish flushing before exiting (no SIGKILL mid-write), so the disk stays clean — give it room
with a generous `stop_grace_period`. Shutting the guest down **from inside** powers it off and it
**stays off** (the `vm` program exits 0; supervisord's `autorestart=unexpected` only restarts a
*crash*) — boot it again with `sctl start vm` or by restarting the container. A guest *reboot* just
resets the VM in place.

---

## Console, KVM, logs

- **Console**: open `http://<host>:8006/` for the noVNC browser console (full mouse/keyboard).
  Every guest exposes its display on QEMU VNC `:0`; `novnc` bridges it.
- **KVM**: pass `--device=/dev/kvm`. Each starter probes it and uses `-cpu host` with
  `accel=kvm`; without it QEMU falls back to `accel=tcg` + `-cpu max` (software emulation — far too
  slow for a real install). The warning is logged.
- **Logs**: supervisord writes `/var/log/supervisord-vm.log` (QEMU / install / boot),
  `/var/log/supervisord-tpm.log` (vTPM) and `/var/log/supervisord-novnc.log` (console bridge) —
  inside the container (not on the `/storage` volume). Tail them with
  `docker exec <c> tail -f /var/log/supervisord-vm.log`.

---

## vTPM (the `tpm` service)

Guests that require a TPM 2.0 (e.g. Windows 11) get one from **`swtpm`**, run as its own supervisord
program (`start-tpm`) rather than launched inline — so it can be controlled on its own
(`sctl start|stop|restart tpm`) and reused by any guest. Like every service it is toggled with the
ubase `ZSRV_*` switch: **`ZSRV_tpm`** (off until enabled, like every service — set `ZSRV_tpm=true`
for a TPM guest such as Windows 11). There is no separate TPM env knob.

State lives **next to the guest disk** (`<disk>.tpm/`, control socket `<disk>.tpm/swtpm-sock`),
derived from the disk path like all other per-VM state — so it persists across restarts and each
clone is isolated. The `tpm` service starts before `vm` (lower supervisord priority); because
programs start in parallel, the guest starter **waits** for the vTPM socket before booting QEMU. It
carries no swtpm specifics — it just delegates: `start-tpm socket` returns the control-socket path
(for `-readconfig`) and `start-tpm wait` starts the service if needed and blocks until it is ready,
failing fast rather than booting Windows without a TPM. A new TPM-needing guest enables `ZSRV_tpm`
and calls `start-tpm wait`; guests that need no TPM just leave it off.

---

## Per-VM config & state convention

Every guest derives its per-VM files from the **disk path**: with `STATE="${DISK%.*}"`, a disk at
`/storage/win11/windows.qcow2` yields `windows.conf`, `windows.qemu.conf`, `windows.OVMF_VARS.fd`,
`windows.tpm/`, `windows.monitor.sock`, etc. — all next to the disk. Point `DISK` at a different
path (e.g. a clone) and it gets its own independent state automatically; many VMs share one
`/storage` without colliding.

The config file is **incus-style**: on first boot the starter seeds `<disk>.conf` from the
environment, then treats that file as authoritative. **Edit it and restart** to change runtime
settings; delete it to re-seed from the environment. Identity/locale that is baked at install time
(account, language, edition) is **not** re-read from the conf.

State files a guest may keep next to its disk:

| File | Meaning |
|------|---------|
| `<disk>.conf` | editable per-VM config (authoritative after first boot) |
| `<disk>.qemu.conf` | generated `-readconfig` topology (regenerated each boot; do not edit) |
| `<disk>.install` | install lifecycle record (`installing` / `installed`) — gates re-install |
| `<disk>.force-applied` | one-shot guard for `WIN11_INSTALL=force` |
| `<disk>.OVMF_VARS.fd`, `<disk>.tpm/` | UEFI NVRAM and swtpm state (firmware-class guests) |

---

## Storage layout

```
storage/
├── <os>/        # per-guest state: the qcow2 + all <disk>.* files + clones/ + base.qcow2
├── logs/        # vm.log, novnc.log  (shared)
└── …
```

One directory per guest (`storage/win11/`, `storage/alpine/`). Transient build artifacts (e.g. the
repacked Windows install ISO) go to `/tmp`, **never** into `/storage`. Mount input ISOs read-only
under `/images`.

---

## Adding a guest

The engine, console, networking and device passthrough are all guest-agnostic, so a new guest is
small — it owns its boot logic and shares the generic helpers from `qemu-common.sh`:

1. **Write `rootfs/build/bin/start-<os>`** — a script that sources `qemu-common.sh` (after setting
   `LOG_TAG`) and:
   - reads its knobs from env (`RAM_SIZE`, `CPU_CORES`, `DISK`, `NETWORK`, …);
   - defaults `DISK` to `$STORAGE/<os>/<name>.qcow2` and `mkdir -p "$(dirname "$DISK")"` (keep the
     per-OS dir convention);
   - probes KVM with the shared `detect_kvm`, sets up VNC with `build_vnc_args`, builds its QEMU
     command, and (optionally) downloads/locates its install media under `/images` or `/storage`;
   - launches QEMU in the background and hands the pid to `supervise_qemu` (graceful stop + stay-off);
   - puts transient build artifacts in `/tmp`, not `/storage`.
2. **Wire it into `start-vm`** — add a `case` arm mapping your `OS` value(s) to `start-<os>`.
3. **Reuse the commons** — networking and device passthrough work the same; see
   [networking-and-devices.md](./networking-and-devices.md). If your guest installs unattended,
   consider an install-state file (`<disk>.install`) like Windows uses.
4. **Document it** — add `docs/guests/<os>.md` (+ `.zh-CN.md`) and list it in
   [docs/README.md](../README.md).

[Alpine](../guests/alpine.md) is the minimal reference (console install, SeaBIOS, no unattended
pipeline); [Windows](../guests/windows.md) is the full example (unattended install, TPM/UEFI,
slipstreamed drivers, install-state). Document the new guest from
[`docs/guests/_template.md`](../guests/_template.md).

### Starter skeleton

A minimal `rootfs/build/bin/start-<os>` to copy from:

```bash
#!/usr/bin/env bash
# start-<os> — minimal guest starter. Shares helpers from qemu-common.sh; wire OS=<os> into start-vm.
set -euo pipefail
LOG_TAG=<os>
source /build/bin/qemu-common.sh
: "${STORAGE:=/storage}"; : "${IMAGES:=/images}"
: "${RAM_SIZE:=2G}"; : "${CPU_CORES:=2}"; : "${DISK_SIZE:=16G}"; : "${MACHINE:=q35}"
: "${DISK:=$STORAGE/<os>/<os>.qcow2}"          # per-OS dir convention
: "${NETWORK:=user}"; : "${PORT_FWD:=2222-22}"; : "${EXTRA_ARGS:=}"
: "${VNC_HOST:=127.0.0.1}"; : "${VNC_PASSWORD:=}"
STATE="${DISK%.*}"; MONITOR="$STATE.monitor.sock"; VNC_SECRET="/tmp/$(basename "$STATE").vncpw"

mkdir -p "$(dirname "$DISK")"
detect_kvm; ACCEL=(-machine "${MACHINE},accel=$ACCEL_MODE" -cpu "$CPU_MODEL")   # from qemu-common.sh
build_vnc_args                                                                  # -> VNC_ARGS
[ -f "$DISK" ] || qemu-img create -f qcow2 "$DISK" "$DISK_SIZE" >/dev/null
# TODO: locate/download install media under /images or /storage; build artifacts -> /tmp.
rm -f "$MONITOR"

# shellcheck disable=SC2086
qemu-system-x86_64 "${ACCEL[@]}" -smp "$CPU_CORES" -m "$RAM_SIZE" \
    -device virtio-rng-pci -device VGA "${VNC_ARGS[@]}" \
    -monitor "unix:$MONITOR,server,nowait" -name "<os>" \
    -netdev "user,id=net0$(user_hostfwd "$PORT_FWD")" -device virtio-net-pci,netdev=net0 $EXTRA_ARGS \
    -drive "file=$DISK,if=none,id=disk0,format=qcow2,cache=writeback" \
    -device virtio-blk-pci,drive=disk0,bootindex=1 &
supervise_qemu $!   # graceful stop + stay-off (from qemu-common.sh)
```

Then add a `case` arm to `start-vm`:

```bash
    <os>|<alias>)
        log "OS=$OS -> start-<os>"; exec /build/bin/start-<os> ;;
```
