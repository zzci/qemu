# 网络与设备直通

[English](./networking-and-devices.md) · **中文**

如何为客户机配置网卡、USB 设备和串口。以下对 `zzci/qemu` 引擎通用;示例以 Windows 客户机为例。概览见
[根 README](../../README.zh-CN.md),Windows 细节见 [windows.zh-CN.md](../guests/windows.zh-CN.md)。

几个旋钮覆盖常见场景 —— `NETWORK`、`USB`、`SERIAL` 和原始 `EXTRA_ARGS` 逃生口 —— 再配上对应的 `docker` 设备/权限参数,
让宿主资源在容器内可见。

---

## 网络

用 `NETWORK` 选择模式。每块客户机网卡在系统里显示为一个适配器。

| 模式 | 行为 | 容器要求 |
|------|------|----------|
| `user`(默认) | QEMU 用户态(SLIRP)NAT;出站可用 + `PORT_FWD` 宿主→客户机端口转发 | 无 |
| `bridge` | 在已有宿主网桥 `$BRIDGE` 上建 `tap`;客户机加入该二层(从局域网 DHCP) | `--cap-add NET_ADMIN`、`--device=/dev/net/tun`、网桥已存在 |
| `host` | 同 `bridge`,用于 `docker run --network host` + 宿主网桥 | 同 `bridge` |
| `macvlan` | 在 `$MACVLAN` 网卡上建 `macvtap`;客户机经 DHCP 获得自己的局域网 IP | `--cap-add NET_ADMIN`、`--device-cgroup-rule='c *:* rwm'`、一个 docker `macvlan` 网络 |
| `none` | 无网卡 | 无 |

客户机 MAC 由磁盘路径派生(跨重启与克隆保持稳定,DHCP 租约不变)。`user` 模式经容器访问 RDP;
`bridge`/`macvlan` 模式客户机有真实局域网 IP,直接 RDP/SSH 即可。

### user(NAT)—— 默认

`PORT_FWD` 用 `host-guest` 对(逗号分隔)列出宿主↔客户机转发 —— 暴露客户机端口的通用机制(仅 `user`
模式有意义)。每个客户机有合理默认(win11 `3389-3389` 即 RDP,alpine `2222-22` 即 SSH);需要时覆盖成任意映射。

```yaml
environment:
  NETWORK: "user"
  PORT_FWD: "3389-3389,8080-80"   # 宿主 3389 -> 客户机 3389(RDP),宿主 8080 -> 客户机 80
ports:
  - "3389:3389"                   # 转发了的宿主端口也要发布出来
  - "8080:8080"
```

### bridge

```bash
docker run -d --name win11 --device=/dev/kvm --device=/dev/net/tun --cap-add NET_ADMIN \
  -e NETWORK=bridge -e BRIDGE=br0 \
  -v "$PWD/storage:/storage" zzci/qemu
```

### macvlan(客户机获得真实局域网 IP)

```bash
docker network create -d macvlan \
  --subnet=192.168.1.0/24 --gateway=192.168.1.1 -o parent=enp1s0 lan

docker run -d --name win11 --network lan \
  --device=/dev/kvm --device=/dev/net/tun \
  --cap-add NET_ADMIN --device-cgroup-rule='c *:* rwm' \
  -e NETWORK=macvlan -e MACVLAN=eth0 \
  -v "$PWD/storage:/storage" zzci/qemu
```

`MACVLAN` 接受逗号列表以创建**多块** macvtap 网卡(`MACVLAN=eth0,eth1`)。

> macvlan 注意:设计上**宿主**无法直接访问 macvlan 客户机 IP(同局域网其它机器可以)。若需宿主↔客户机,
> 在宿主上加一个 macvlan 子接口。

### 同时两块网卡(如 macvlan + 一块用于宿主访问的 NAT 网卡)

`NETWORK` 只选一种模式。要加第二块网卡,用 `EXTRA_ARGS` 追加原始 netdev —— 用**不同的 id 和 MAC**
(内置网卡用 `net0`/`net1…`):

```yaml
environment:
  NETWORK: "macvlan"
  MACVLAN: "eth0"
  EXTRA_ARGS: "-netdev user,id=usernet,hostfwd=tcp::3389-:3389 -device virtio-net-pci,netdev=usernet,mac=52:54:00:12:34:56"
```

这样客户机既经 macvlan 拿到局域网 IP,**又**有一块 NAT 网卡,其 `3389` 可从宿主访问 —— 正好绕开 macvlan
的宿主↔客户机限制。Windows 按接口 metric 选路。

---

## USB 直通

按 **vendor:product**(十六进制)直通宿主 USB 设备,逗号分隔:

```yaml
environment:
  USB: "0bda:8153,1d6b:0002"
```

```bash
docker run -d --name win11 --device=/dev/kvm --device=/dev/bus/usb \
  -e USB=0bda:8153 -v "$PWD/storage:/storage" zzci/qemu
```

每一项生成一个 `-device usb-host,vendorid=0x…,productid=0x…`,挂到客户机的 xHCI 控制器。用宿主上的
`lsusb` 查 ID(`1d6b:0002` 这种形式)。

**要求与注意**

- 宿主设备必须在容器内可见:`--device=/dev/bus/usb`(或 `--privileged`)。
- 按 USB ID 匹配,且发生在**启动时** —— 不支持热插拔,两个同型号设备会有歧义。要从多个同型号设备中指定一个,
  用 `EXTRA_ARGS` 按总线/端口寻址:

  ```yaml
  EXTRA_ARGS: "-device usb-host,hostbus=1,hostport=4"   # 总线/端口见 `lsusb -t`
  ```
- **USB 转串口适配器**(FTDI / CP210x / CH340 等)通常最好按 USB 直通,让客户机加载自己的驱动并暴露 COM 口 ——
  见下一节。

---

## 串口

按来源分三种做法:

### 1. USB 转串口适配器 —— 按 USB 直通(推荐)

让客户机接管适配器,用其原生驱动创建 COM 口:

```bash
docker run -d --device=/dev/kvm --device=/dev/bus/usb \
  -e USB=0403:6001 ...        # 例如一个 FTDI 适配器
```

### 2. 真实宿主串口 —— `SERIAL`(一等能力)

`SERIAL` 接受逗号分隔的宿主 TTY 路径,每个变成客户机的一个 `pci-serial` COM 口(`ser0`、`ser1`……)。
用 `--device=` 把 TTY 暴露给容器:

```yaml
environment:
  SERIAL: "/dev/ttyUSB0,/dev/ttyS0"
devices:
  - /dev/ttyUSB0
  - /dev/ttyS0               # 每个 TTY 都必须在容器内可见
```

```bash
docker run -d --device=/dev/kvm --device=/dev/ttyUSB0 -e SERIAL=/dev/ttyUSB0 ...
```

`pci-serial` 是现代 PCIe COM 口。要传统的 COM1/COM2(ISA)或 socket/telnet 后端,用 `EXTRA_ARGS`(见下)。

### 3. 其它 —— `EXTRA_ARGS`

```yaml
# 传统 ISA COM 口
EXTRA_ARGS: "-chardev serial,id=ser0,path=/dev/ttyUSB0 -device isa-serial,chardev=ser0"
# 把串口经网络暴露(用 telnet 连);用 -p 7000:7000 发布
EXTRA_ARGS: "-chardev socket,id=ser0,host=0.0.0.0,port=7000,server=on,wait=off -device pci-serial,chardev=ser0"
```

> `SERIAL` 用的 id 是 `ser0`、`ser1`……;若你同时用 `EXTRA_ARGS` 加串口设备,请用不同 id 以免冲突。

---

## `EXTRA_ARGS` 万能逃生口

`EXTRA_ARGS` 原样追加到 QEMU 命令行(并存入每机 conf),所以 QEMU 支持但没有专用旋钮的东西都能加在这:
额外网卡、串口/并口、额外磁盘、PCI 直通(`vfio-pci`)、自定义 `-device` 拓扑等。务必带上对应的 `docker`
参数(`--device=…`、`--cap-add …`、`--device-cgroup-rule=…`),让宿主资源在容器内可达。

```yaml
environment:
  EXTRA_ARGS: "-drive file=/storage/data.qcow2,if=none,id=d1,format=qcow2 -device virtio-blk-pci,drive=d1"
```

> 这些直通配方是配置参考 —— 具体设备路径、ID 和宿主能力因机器而异,请在你的主机上验证。
