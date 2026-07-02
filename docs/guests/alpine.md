# Alpine Linux guest (`OS=alpine`)

**English** · [中文](./alpine.zh-CN.md)

A small, console-installed Linux guest. It exercises the `zzci/qemu` engine end to end **without**
an unattended pipeline — you install Alpine interactively from the browser console. Handy as a
quick sanity check that KVM, VNC and networking work, or as a tiny Linux VM in its own right.

See the [root README](../../README.md) for the engine overview.

---

## How it works

`start-alpine` boots the Alpine ISO next to a blank disk and exposes VNC. Unlike Windows there is
**no OVMF/TPM** (SeaBIOS is enough) and **no media repack** — the whole install is driven from the
console. A blank disk has no boot sector, so `bootindex=1` falls through to the ISO (`bootindex=2`)
for the live installer; once installed, the disk boots first.

The ISO is resolved as `ALPINE_ISO` → `/images/alpine.iso` → `/storage/alpine.iso`, else downloaded
(`alpine-virt`, small & stable CDN). State lives under `storage/alpine/`.

---

## Install

```bash
docker run -d --name alpine --device=/dev/kvm \
  -e ZSRV_vm=true -e ZSRV_novnc=true -e OS=alpine \
  -v "$PWD/storage-alpine:/storage" -p 8006:8006 -p 2222:2222 zzci/qemu
```

Then open the browser console and install interactively:

```
open  http://localhost:8006/

login: root            # no password
setup-alpine           # interactive setup (keyboard, hostname, network, disk…)
setup-disk -m sys /dev/vda    # install to the disk (sys mode)
poweroff               # powers off; the container restarts it, now booting from disk
```

After `poweroff` the `vm` service restarts the VM; with the OS now on `/dev/vda` it boots from disk
instead of the ISO. SSH is forwarded by `PORT_FWD` (default `2222-22`, i.e. host `2222` → guest `22`).

---

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OS` | `win11` | Set to `alpine` to select this guest. |
| `RAM_SIZE` | `2G` | Guest memory. |
| `CPU_CORES` | `2` | vCPUs. |
| `DISK_SIZE` | `8G` | Disk size (first run only). |
| `DISK` | `/storage/alpine/alpine.qcow2` | Disk path. |
| `NETWORK` | `user` | `user` (SLIRP NAT + port forwards) or `none`. |
| `PORT_FWD` | `2222-22` | user-mode host-guest forwards, comma list (e.g. `2222-22,8080-80`). |
| `VGA` | `std` | `std` or `virtio` display adapter. |
| `CONSOLE` | `off` | `on` exposes the text console (ttyS0) for `vm-console` (see below). |
| `ALPINE_VERSION` | `3.21` | Release to download when no local ISO. |
| `ALPINE_ISO` | – | Explicit ISO path (skips the lookup/download). |
| `EXTRA_ARGS` | – | Raw extra QEMU arguments. |

For networking, USB and serial passthrough, the
[networking-and-devices.md](../common/networking-and-devices.md) guide applies here too (it is
guest-agnostic). Alpine uses `user`/`none` only out of the box; for `bridge`/`macvlan` adapt the
netdev via `EXTRA_ARGS`.

---

## Text console (`vm-console`)

Besides the noVNC graphical console, run with `CONSOLE=on` to get a **serial text console** you can
attach a terminal to — handy for a headless Linux VM:

```bash
docker exec -it alpine vm-console        # detach with Ctrl-]
```

Alpine must put a login on `ttyS0` for this to show anything. The simplest way: during
`setup-alpine` answer the **serial port** prompt (or afterwards add `ttyS0` to `/etc/inittab` and
`console=ttyS0` to the bootloader). Then `vm-console` gives you boot messages and a login over the
terminal.

---

## Adding another guest

Alpine is the template for non-Windows guests: drop a `start-<os>` script in `rootfs/build/bin/`
that builds its own QEMU command, then add a `case` to `start-vm`. The engine, console, networking
and device passthrough are all guest-agnostic.
