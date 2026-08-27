# Networking

**English** · [中文](./networking.zh-CN.md)

Part of the [device guides](./devices.md) — everything the guest sees is built by its launcher
(`{dir}/scripts/launcher`); edit that file and power-cycle the VM to apply.

---

## 1. User mode / NAT (default)

The templates ship QEMU's user-mode network — zero host setup, outbound NAT, per-port inbound
forwards:

```bash
-netdev user,id=net0,hostfwd=tcp::3389-:3389 \
-device "virtio-net-pci,netdev=net0,mac=${VMD_MAC}"
```

- Add forwards by appending `hostfwd=` segments (comma-separated, host-port`-:`guest-port):
  `hostfwd=tcp::3389-:3389,hostfwd=tcp::2222-:22,hostfwd=udp::5353-:53`
- Then publish the host side from Docker (`-p 127.0.0.1:3389:3389`, drop the prefix only for
  trusted networks). Forwards are shown on the console home screen (parsed from the launcher +
  the optional `PORT_FWD="3389-3389,2222-22"` env).
- Pros: works everywhere. Cons: guest is NATed (no inbound except forwards), slightly slower.

## 2. Bridged / tap (guest on a real L2 segment)

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

## 3. Multiple NICs

Repeat the pair with distinct ids; derive extra MACs from the stable `VMD_MAC` or hardcode:

```bash
-netdev user,id=net0,hostfwd=tcp::3389-:3389 -device virtio-net-pci,netdev=net0,mac=${VMD_MAC} \
-netdev tap,id=net1,ifname=tap0,script=no,downscript=no -device virtio-net-pci,netdev=net1,mac=52:54:00:aa:bb:01
```

`VMD_MAC` is derived from the disk path, so a guest keeps its MAC (and DHCP lease) across
restarts.
