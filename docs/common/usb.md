# USB

**English** · [中文](./usb.zh-CN.md)

Part of the [device guides](./devices.md) — everything the guest sees is built by its launcher
(`{dir}/scripts/launcher`); edit that file and power-cycle the VM to apply.

---

The win11 template already provides a USB controller + tablet:

```bash
-device qemu-xhci -device usb-tablet
```

## Host USB passthrough

1. Give the container the USB bus (compose):

```yaml
devices:
  - /dev/kvm
  - /dev/bus/usb            # whole bus; or a single /dev/bus/usb/BBB/DDD
```

2. Attach by vendor/product (survives replug) or by bus/port (fixed physical port):

```bash
-device usb-host,vendorid=0x046d,productid=0xc52b     # lsusb → ID 046d:c52b
-device usb-host,hostbus=1,hostport=2
```

> **Order matters** — `usb-host` (anything needing a USB bus) must come **after** `-device qemu-xhci`
> on the command line. QEMU realizes `-device` in order, so a `usb-host` placed *before* the
> controller fails with `No 'usb-bus' bus found for device 'usb-host'`. Put it in the runtime
> **launcher** (which has `qemu-xhci`), **not** the lean install phase (which has no USB controller).
> With a single controller the plain `vendorid/productid` auto-attaches — an explicit `bus=xhci.0`
> (and `id=xhci` on the controller) is only needed to disambiguate *multiple* controllers.

USB3 devices just work through qemu-xhci. For isochronous devices (audio, webcams) results vary —
prefer bus/port attachment.

## USB storage from an image file

```bash
-drive file=/vms/win11/usbdisk.img,if=none,id=usb1,format=raw \
-device usb-storage,drive=usb1
```
