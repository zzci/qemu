# 网络

[English](./networking.md) · **中文**

本文属于[设备指南](./devices.zh-CN.md)——客户机看到的一切设备都由它的 launcher
(`{dir}/scripts/launcher`)拼出;改完该文件后重启 VM 生效。

---

## 1. 用户模式 / NAT(默认)

模板自带 QEMU 用户态网络——零宿主配置、出站 NAT、按端口做入站转发:

```bash
-netdev user,id=net0,hostfwd=tcp::3389-:3389 \
-device "virtio-net-pci,netdev=net0,mac=${VMD_MAC}"
```

- 追加 `hostfwd=` 段即可增加转发(逗号分隔,宿主端口`-:`客户机端口):
  `hostfwd=tcp::3389-:3389,hostfwd=tcp::2222-:22,hostfwd=udp::5353-:53`
- 再由 Docker 发布宿主端口(`-p 127.0.0.1:3389:3389`,仅在可信网络才去掉前缀)。转发会显示在控制台
  首页(解析自 launcher 与可选的 `PORT_FWD="3389-3389,2222-22"` 环境变量)。
- 优点:处处可用。缺点:客户机在 NAT 后(除转发外无入站),性能略低。

## 2. 桥接 / tap(客户机上真实二层)

给容器 `--cap-add NET_ADMIN --device /dev/net/tun`,用 `vmd.toml` 的 `prepare` 钩子在启动前建好
桥和 tap,再把 QEMU 指到 tap:

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

`br0` 接到哪里决定可达性:把容器的 `eth0` 加入桥,或让容器跑在 Docker **macvlan** 网络上,客户
机即可直接获得局域网地址:

```bash
docker network create -d macvlan --subnet 192.168.1.0/24 --gateway 192.168.1.1 \
  -o parent=eth0 lan
docker run --network lan --cap-add NET_ADMIN --device /dev/net/tun ... zzci/qemu
```

(容器内把 `eth0` 与 `tap0` 桥起来,客户机直接向局域网 DHCP。)

## 3. 多网卡

重复 netdev/device 对,id 区分;额外 MAC 可自定或从稳定的 `VMD_MAC` 派生:

```bash
-netdev user,id=net0,hostfwd=tcp::3389-:3389 -device virtio-net-pci,netdev=net0,mac=${VMD_MAC} \
-netdev tap,id=net1,ifname=tap0,script=no,downscript=no -device virtio-net-pci,netdev=net1,mac=52:54:00:aa:bb:01
```

`VMD_MAC` 由磁盘路径推导,重启后 MAC(及 DHCP 租约)保持不变。
