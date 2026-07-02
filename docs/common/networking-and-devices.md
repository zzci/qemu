# Networking & device passthrough

**English** · [中文](./networking-and-devices.zh-CN.md)

How to give a guest network adapters, USB devices and serial ports. These apply to the `zzci/qemu`
engine in general; the examples use the Windows guest. See the [root README](../../README.md) for the
overview and [windows.md](../guests/windows.md) for the Windows specifics.

A few knobs cover the common cases — `NETWORK`, `USB`, `SERIAL`, and the raw `EXTRA_ARGS` escape hatch — plus
the matching `docker` device/cap flags so the host resource is visible inside the container.

---

## Networking

Select the mode with `NETWORK`. Each guest NIC shows up as one adapter in the guest.

| Mode | Behavior | Container requirements |
|------|----------|------------------------|
| `user` (default) | QEMU user-mode (SLIRP) NAT; outbound works + `PORT_FWD` host→guest port forwards | none |
| `bridge` | a `tap` on an existing host bridge `$BRIDGE`; guest joins that L2 (DHCP from the LAN) | `--cap-add NET_ADMIN`, `--device=/dev/net/tun`, the bridge present |
| `host` | same as `bridge`, intended for `docker run --network host` + a host bridge | as `bridge` |
| `macvlan` | a `macvtap` on `$MACVLAN` iface(s); guest gets its own LAN IP via DHCP | `--cap-add NET_ADMIN`, `--device-cgroup-rule='c *:* rwm'`, a docker `macvlan` network |
| `none` | no network adapter | none |

The guest MAC is derived from the disk path (stable across restarts and clones, so DHCP leases stay
put). In `user` mode you reach RDP through the container; in `bridge`/`macvlan` the guest has a real
LAN IP, so RDP/SSH straight to it.

### user (NAT) — the default

`PORT_FWD` lists host↔guest forwards as `host-guest` pairs, comma-separated — the generic mechanism
for exposing guest ports (only meaningful in `user` mode). Each guest sets a sensible default
(win11 `3389-3389` for RDP, alpine `2222-22` for SSH); override it to map whatever you need.

```yaml
environment:
  NETWORK: "user"
  PORT_FWD: "3389-3389,8080-80"   # host 3389 -> guest 3389 (RDP), host 8080 -> guest 80
ports:
  - "3389:3389"                   # also publish the host ports you forwarded
  - "8080:8080"
```

### bridge

```bash
docker run -d --name win11 --device=/dev/kvm --device=/dev/net/tun --cap-add NET_ADMIN \
  -e NETWORK=bridge -e BRIDGE=br0 \
  -v "$PWD/storage:/storage" zzci/qemu
```

### macvlan (guest gets a real LAN IP)

```bash
docker network create -d macvlan \
  --subnet=192.168.1.0/24 --gateway=192.168.1.1 -o parent=enp1s0 lan

docker run -d --name win11 --network lan \
  --device=/dev/kvm --device=/dev/net/tun \
  --cap-add NET_ADMIN --device-cgroup-rule='c *:* rwm' \
  -e NETWORK=macvlan -e MACVLAN=eth0 \
  -v "$PWD/storage:/storage" zzci/qemu
```

`MACVLAN` accepts a comma list for **multiple** macvtap NICs (`MACVLAN=eth0,eth1`).

> macvlan caveat: by design the **host** cannot reach a macvlan guest IP directly (other LAN
> machines can). Add a macvlan sub-interface on the host if you need host↔guest.

### Two NICs at once (e.g. macvlan + a NAT NIC for host access)

`NETWORK` selects a single mode. To add a second adapter, append a raw netdev via `EXTRA_ARGS` —
use a **distinct id and MAC** (the built-in NICs use `net0`/`net1…`):

```yaml
environment:
  NETWORK: "macvlan"
  MACVLAN: "eth0"
  EXTRA_ARGS: "-netdev user,id=usernet,hostfwd=tcp::3389-:3389 -device virtio-net-pci,netdev=usernet,mac=52:54:00:12:34:56"
```

This gives the guest its LAN IP via macvlan **and** a NAT NIC whose `3389` is reachable from the
host — handy because macvlan otherwise blocks host↔guest. Windows picks the route by interface metric.

---

## USB passthrough

Pass host USB devices by **vendor:product** (hex), comma-separated:

```yaml
environment:
  USB: "0bda:8153,1d6b:0002"
```

```bash
docker run -d --name win11 --device=/dev/kvm --device=/dev/bus/usb \
  -e USB=0bda:8153 -v "$PWD/storage:/storage" zzci/qemu
```

Each entry becomes a `-device usb-host,vendorid=0x…,productid=0x…` attached to the guest's xHCI
controller. Find IDs on the host with `lsusb` (the `1d6b:0002` form).

**Requirements & caveats**

- The host device must be visible inside the container: `--device=/dev/bus/usb` (or `--privileged`).
- Matching is by USB ID and happens **at boot** — no hotplug, and two identical devices are
  ambiguous. To pick one of several identical devices, address it by bus/port via `EXTRA_ARGS`:

  ```yaml
  EXTRA_ARGS: "-device usb-host,hostbus=1,hostport=4"   # see `lsusb -t` for bus/port
  ```
- A **USB-serial adapter** (FTDI / CP210x / CH340 / …) is usually best passed through as USB so the
  guest loads its own driver and exposes a COM port — see the next section.

---

## Serial ports

Three approaches, by source:

### 1. A USB-serial adapter — pass it as USB (recommended)

Let the guest own the adapter and create the COM port with its native driver:

```bash
docker run -d --device=/dev/kvm --device=/dev/bus/usb \
  -e USB=0403:6001 ...        # e.g. an FTDI adapter
```

### 2. A real host serial port — `SERIAL` (first-class)

`SERIAL` takes a comma list of host TTY paths; each becomes a guest `pci-serial` COM port
(`ser0`, `ser1`, …). Expose the TTY to the container with `--device=`:

```yaml
environment:
  SERIAL: "/dev/ttyUSB0,/dev/ttyS0"
devices:
  - /dev/ttyUSB0
  - /dev/ttyS0               # each TTY must be visible in the container
```

```bash
docker run -d --device=/dev/kvm --device=/dev/ttyUSB0 -e SERIAL=/dev/ttyUSB0 ...
```

`pci-serial` is a modern PCIe COM port. For the legacy COM1/COM2 ISA range, or a socket/telnet
backend, use `EXTRA_ARGS` instead (next).

### 3. Anything else — `EXTRA_ARGS`

```yaml
# legacy ISA COM port
EXTRA_ARGS: "-chardev serial,id=ser0,path=/dev/ttyUSB0 -device isa-serial,chardev=ser0"
# expose a port over the network (telnet to it); publish 7000 with -p 7000:7000
EXTRA_ARGS: "-chardev socket,id=ser0,host=0.0.0.0,port=7000,server=on,wait=off -device pci-serial,chardev=ser0"
```

> `SERIAL` uses ids `ser0`, `ser1`, …; if you also add serial devices via `EXTRA_ARGS`, give them
> distinct ids to avoid a clash.

---

## The `EXTRA_ARGS` escape hatch

`EXTRA_ARGS` is appended verbatim to the QEMU command line (and stored in the per-VM conf), so
anything QEMU supports that has no dedicated knob can be added here: extra NICs, serial/parallel
ports, additional disks, PCI passthrough (`vfio-pci`), custom `-device` topology, etc. Always
remember the matching `docker` flag (`--device=…`, `--cap-add …`, `--device-cgroup-rule=…`) so the
host resource is reachable inside the container.

```yaml
environment:
  EXTRA_ARGS: "-drive file=/storage/data.qcow2,if=none,id=d1,format=qcow2 -device virtio-blk-pci,drive=d1"
```

> These passthrough recipes are configuration references — exact device paths, IDs and host
> capabilities vary by machine, so verify on your host.
