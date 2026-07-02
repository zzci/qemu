# 网络、串口、USB 与设备

[English](./networking-and-devices.md) · **中文**

客户机看到的一切设备都由**它的 launcher 脚本**决定 —— `{dir}/scripts/launcher`,首启从模板播种
的可编辑副本。改网络、串口、USB 或任何设备:编辑该文件,然后重启 VM(`shutdown` + `start`,或
重启容器)。`vmd print` 可在不运行的情况下查看解析后的完整命令。

launcher 通过 `VMD_*` 环境变量拿到参数(`VMD_MAC`、`VMD_QMP`、`VMD_CONSOLE_SOCK` 等)——见
[engine.zh-CN.md](./engine.zh-CN.md)。

---

## 网络

### 1. 用户模式 / NAT(默认)

模板自带 QEMU 用户态网络——零宿主配置、出站 NAT、按端口做入站转发:

```bash
-netdev user,id=net0,hostfwd=tcp::3389-:3389 \
-device "virtio-net-pci,netdev=net0,mac=${VMD_MAC}"
```

- 追加 `hostfwd=` 段即可增加转发(逗号分隔,宿主端口`-:`客户机端口):
  `hostfwd=tcp::3389-:3389,hostfwd=tcp::2222-:22,hostfwd=udp::5353-:53`
- 再由 Docker 发布宿主端口(`-p 127.0.0.1:3389:3389`,仅在可信网络才去掉前缀)。转发会显示在控制台首页(解析自 launcher 与可选
  的 `PORT_FWD="3389-3389,2222-22"` 环境变量)。
- 优点:处处可用。缺点:客户机在 NAT 后(除转发外无入站),性能略低。

### 2. 桥接 / tap(客户机上真实二层)

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

### 3. 多网卡

重复 netdev/device 对,id 区分;额外 MAC 可自定或从稳定的 `VMD_MAC` 派生:

```bash
-netdev user,id=net0,hostfwd=tcp::3389-:3389 -device virtio-net-pci,netdev=net0,mac=${VMD_MAC} \
-netdev tap,id=net1,ifname=tap0,script=no,downscript=no -device virtio-net-pci,netdev=net1,mac=52:54:00:aa:bb:01
```

`VMD_MAC` 由磁盘路径推导,重启后 MAC(及 DHCP 租约)保持不变。

---

## 串口

### 内置串口控制台

把 COM1 接到 vmd 的控制台 socket,Web 终端(`/console`)即可用:

```bash
-serial "unix:${VMD_CONSOLE_SOCK},server,nowait"
```

Alpine 模板默认如此(`console=ttyS0`)。Windows 用处有限,win11 模板未接——需要 COM1 就自行加上。

### 串口映射到 TCP

```bash
-serial tcp:0.0.0.0:4555,server,nowait      # 容器内的裸 TCP 服务
# telnet 形式:-serial telnet:0.0.0.0:4555,server,nowait
```

配合 `-p 127.0.0.1:4555:4555` 发布。任何连上该端口的连接都直通客户机 COM 口。

### 直通宿主机串口设备

先把设备映射进容器,再交给 QEMU:

```yaml
devices:
  - /dev/kvm
  - /dev/ttyUSB0            # 物理串口适配器
```

```bash
-serial /dev/ttyUSB0
```

### 多个 COM 口

每个 `-serial …` 依次是 COM1、COM2……;`-serial null` 跳过一个槽位。更多端口用
`-device pci-serial` 搭配显式 chardev。

---

## USB

win11 模板已带 USB 控制器和平板指针:

```bash
-device qemu-xhci -device usb-tablet
```

### 宿主机 USB 直通

1. 把 USB 总线给容器(compose):

```yaml
devices:
  - /dev/kvm
  - /dev/bus/usb            # 整条总线;或单个 /dev/bus/usb/BBB/DDD
```

2. 按厂商/产品号(拔插后仍生效)或按总线/端口(固定物理口)挂载:

```bash
-device usb-host,vendorid=0x046d,productid=0xc52b     # lsusb → ID 046d:c52b
-device usb-host,hostbus=1,hostport=2
```

USB3 设备经 qemu-xhci 直接可用。等时传输设备(音频、摄像头)效果不一,优先按总线/端口挂载。

### 镜像文件模拟 U 盘

```bash
-drive file=/vms/win11/usbdisk.img,if=none,id=usb1,format=raw \
-device usb-storage,drive=usb1
```

---

## 其他设备

- **显示**:模板使用 `-device "virtio-vga,xres=1920,yres=1080"`,改数字即改默认分辨率(virtio
  GPU 驱动装好后客户机内也能切换)。
- **额外磁盘**:加一对 `-drive file=…,if=none,id=disk1 -device virtio-blk-pci,drive=disk1`;
  镜像在 `prepare` 里创建(`qemu-img create -f qcow2 {dir}/data.qcow2 100G`)。
- **光驱**:`-drive file=/images/tools.iso,if=none,id=cd1,media=cdrom -device ide-cd,drive=cd1,bus=ahci.2`
  (win11 模板已有 `ahci` 控制器)。
- **声音**:`-audiodev none,id=snd0 -device ich9-intel-hda -device hda-output,audiodev=snd0`
  (VNC 不传声音;Windows 建议用 RDP 听声)。

改完先 `vmd print` 检查命令,再重启客户机生效。
