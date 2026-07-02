# Networking, serial, USB & devices

**English** · [中文](./networking-and-devices.zh-CN.md)

Everything the guest sees is built by **its launcher script** — `{dir}/scripts/launcher`, your
editable copy seeded from the template on first boot. To change networking, serial ports, USB or
any device: edit that file and power-cycle the VM (`shutdown` + `start`, or restart the
container). `vmd print` shows the resolved command without running anything.

The launcher receives `VMD_*` env vars (`VMD_MAC`, `VMD_QMP`, `VMD_CONSOLE_SOCK`, …) — see
[engine.md](./engine.md).

---

## Networking

### 1. User mode / NAT (default)

The templates ship QEMU's user-mode network — zero host setup, outbound NAT, per-port inbound
forwards:

```bash
-netdev user,id=net0,hostfwd=tcp::3389-:3389 \
-device "virtio-net-pci,netdev=net0,mac=${VMD_MAC}"
```

- Add forwards by appending `hostfwd=` segments (comma-separated, host-port`-:`guest-port):
  `hostfwd=tcp::3389-:3389,hostfwd=tcp::2222-:22,hostfwd=udp::5353-:53`
- Then publish the host side from Docker (`-p 127.0.0.1:3389:3389`, drop the prefix only for trusted networks). Forwards are shown on the console home
  screen (parsed from the launcher + the optional `PORT_FWD="3389-3389,2222-22"` env).
- Pros: works everywhere. Cons: guest is NATed (no inbound except forwards), slightly slower.

### 2. Bridged / tap (guest on a real L2 segment)

Give the container `--cap-add NET_ADMIN --device /dev/net/tun`, create a bridge + tap before boot
(use the guest's `prepare` hooks in `vmd.toml`), and point QEMU at the tap:

```toml
[guest.win11]
prepare = [
  "ip link add br0 type bridge", "ip link set br0 up",
  "ip tuntap add dev tap0 mode tap", "ip link set tap0 master br0", "ip link set tap0 up",
]
```

```bash
-netdev tap,id=net0,ifname=tap0,script=no,downscript=no \
-device "virtio-net-pci,netdev=net0,mac=${VMD_MAC}"
```

What `br0` connects to decides reachability: attach the container's `eth0` to it, or run the
container on a Docker **macvlan** network so the guest gets an address on your LAN:

```bash
docker network create -d macvlan --subnet 192.168.1.0/24 --gateway 192.168.1.1 \
  -o parent=eth0 lan
docker run --network lan --cap-add NET_ADMIN --device /dev/net/tun ... zzci/qemu
```

(bridge `eth0` + `tap0` inside the container; the guest then DHCPs from your LAN).

### 3. Multiple NICs

Repeat the pair with distinct ids; derive extra MACs from the stable `VMD_MAC` or hardcode:

```bash
-netdev user,id=net0,hostfwd=tcp::3389-:3389 -device virtio-net-pci,netdev=net0,mac=${VMD_MAC} \
-netdev tap,id=net1,ifname=tap0,script=no,downscript=no -device virtio-net-pci,netdev=net1,mac=52:54:00:aa:bb:01
```

`VMD_MAC` is derived from the disk path, so a guest keeps its MAC (and DHCP lease) across
restarts.

---

## Serial ports

### The built-in serial console

Wire COM1 to the vmd console socket and the web terminal (`/console`) works:

```bash
-serial "unix:${VMD_CONSOLE_SOCK},server,nowait"
```

The Alpine template does this by default (`console=ttyS0`). For Windows it is of limited use; the
win11 template leaves it out — add it if you want COM1.

### Map a serial port to TCP

```bash
-serial tcp:0.0.0.0:4555,server,nowait      # raw TCP server inside the container
# telnet flavor: -serial telnet:0.0.0.0:4555,server,nowait
```

Publish with `-p 127.0.0.1:4555:4555`. Anything connecting to that port talks to the guest's COM port.

### Pass through a host serial device

Map the device into the container, then hand it to QEMU:

```yaml
devices:
  - /dev/kvm
  - /dev/ttyUSB0            # the physical adapter
```

```bash
-serial /dev/ttyUSB0
```

### Extra COM ports

Each `-serial …` adds the next COM port (COM1, COM2, …). `-serial null` skips a slot. For many
ports use `-device pci-serial` with explicit chardevs.

---

## USB

The win11 template already provides a USB controller + tablet:

```bash
-device qemu-xhci -device usb-tablet
```

### Host USB passthrough

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

USB3 devices just work through qemu-xhci. For isochronous devices (audio, webcams) results vary —
prefer bus/port attachment.

### USB storage from an image file

```bash
-drive file=/vms/win11/usbdisk.img,if=none,id=usb1,format=raw \
-device usb-storage,drive=usb1
```

---

## Other devices

- **Display**: templates use `-device "virtio-vga,xres=1920,yres=1080"`. Change the numbers for a
  different default resolution (the guest can also switch modes once the virtio GPU driver is in).
- **Extra disks**: add a `-drive file=…,if=none,id=disk1 -device virtio-blk-pci,drive=disk1`
  pair; create the image in `prepare` (`qemu-img create -f qcow2 {dir}/data.qcow2 100G`).
- **CD-ROM**: `-drive file=/images/tools.iso,if=none,id=cd1,media=cdrom -device ide-cd,drive=cd1,bus=ahci.2`
  (the win11 template already has an `ahci` controller).
- **Audio**: `-audiodev none,id=snd0 -device ich9-intel-hda -device hda-output,audiodev=snd0`
  (VNC does not carry audio; use RDP for sound on Windows).

After any change: `vmd print` to sanity-check the command, then power-cycle the guest.
