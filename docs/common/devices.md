# Devices

**English** · [中文](./devices.zh-CN.md)

Everything the guest sees is built by **its launcher script** — `{dir}/scripts/launcher`, your
editable copy seeded from the template on first boot. To change any device: edit that file and
power-cycle the VM (`shutdown` + `start`, or restart the container). `vmd print` shows the
resolved command without running anything.

The launcher receives `VMD_*` env vars (`VMD_MAC`, `VMD_QMP`, `VMD_CONSOLE_SOCK`, …) — see
[engine.md](./engine.md).

> **Don't `#`-comment a device line in the launcher.** The `qemu-system-x86_64 … \` invocation is
> one logical line spanning many physical lines via `\`; after the shell joins them, a `#` comments
> out **everything after it** (dropping `-qmp`, the disk, …) and the VM breaks. To toggle a device,
> delete its line, or gate it with an env-driven bash array like the launcher's `INCOMING=()` block.

## Per-device guides

| Guide | What it covers |
|-------|----------------|
| [networking.md](./networking.md) | User/NAT with port forwards, bridge/tap and macvlan, multiple NICs. |
| [serial.md](./serial.md) | The built-in web serial console, COM → TCP, host serial passthrough, extra COM ports. |
| [usb.md](./usb.md) | The xhci controller + tablet, host passthrough by id or bus/port, USB storage images. |
| [file-sharing.md](./file-sharing.md) | virtiofs `shares` — host directories mounted straight into the guest. |

Everything else is small enough to live here.

## Display

Templates use `-device "virtio-vga,xres=1920,yres=1080"`. Change the numbers for a different
default resolution (the guest can also switch modes once the virtio GPU driver is in). Output goes
to the VNC unix socket the web console bridges; there is no host display.

GPU passthrough (VFIO) is not wired up in the engine yet — it needs host IOMMU setup plus
`--device /dev/vfio/…` on the container, and would get its own guide.

## Extra disks

Add a `-drive file=…,if=none,id=disk1 -device virtio-blk-pci,drive=disk1` pair; create the image
in `prepare` (`qemu-img create -f qcow2 {dir}/data.qcow2 100G`).

## CD-ROM

The runtime launchers have no SATA controller (only the lean install phase does), so add one:

```bash
-device ahci,id=ahci \
-drive file=/images/tools.iso,if=none,id=cd1,media=cdrom \
-device ide-cd,drive=cd1,bus=ahci.0
```

Or hang it off the USB controller the win11 template already has:
`-device usb-storage,drive=cd1`.

## Audio

```bash
-audiodev none,id=snd0 -device ich9-intel-hda -device hda-output,audiodev=snd0
```

VNC does not carry audio; use RDP for sound on Windows.

---

After any change: `vmd print` to sanity-check the command, then power-cycle the guest.
