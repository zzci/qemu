# Serial ports

**English** · [中文](./serial.zh-CN.md)

Part of the [device guides](./devices.md) — everything the guest sees is built by its launcher
(`{dir}/scripts/launcher`); edit that file and power-cycle the VM to apply.

---

## The built-in serial console

Wire COM1 to the vmd console socket and the web terminal (`/console`) works:

```bash
-serial "unix:${VMD_CONSOLE_SOCK},server,nowait"
```

The Alpine template does this by default (`console=ttyS0`). For Windows it is of limited use; the
win11 template leaves it out — add it if you want COM1.

## Map a serial port to TCP

```bash
-serial tcp:0.0.0.0:4555,server,nowait      # raw TCP server inside the container
# telnet flavor: -serial telnet:0.0.0.0:4555,server,nowait
```

Publish with `-p 127.0.0.1:4555:4555`. Anything connecting to that port talks to the guest's COM
port.

## Pass through a host serial device

Map the device into the container, then hand it to QEMU:

```yaml
devices:
  - /dev/kvm
  - /dev/ttyUSB0            # the physical adapter
```

```bash
-serial /dev/ttyUSB0
```

## Extra COM ports

Each `-serial …` adds the next COM port (COM1, COM2, …). `-serial null` skips a slot. For many
ports use `-device pci-serial` with explicit chardevs.
