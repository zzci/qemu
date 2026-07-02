# qemu —— Docker 里的 QEMU/KVM 客户机

[English](./README.md) · **中文**

一个打包成单个 Docker 镜像(`zzci/qemu`)、可在浏览器里操作的小型 **QEMU/KVM 虚拟化引擎**。引擎与
**客户机无关**:用 `OS=` 选客户机,`start-vm` 分派到对应启动器。基于
[`zzci/ubase`](https://hub.docker.com/r/zzci/ubase)(Ubuntu 22.04 + tini + supervisord)。一切 ——
准备介质、安装、引导、克隆 —— 都在**容器内**完成,由环境变量和一个可编辑的 incus 风格每机配置驱动。

引擎特性:浏览器控制台(noVNC)· KVM 加速 · 可插拔网络(NAT / bridge / macvlan / host)·
USB 与串口直通 · 每客户机持久存储 · 免重装克隆。

📚 **指南:** [引擎](./docs/common/engine.zh-CN.md) ·
[网络与设备](./docs/common/networking-and-devices.zh-CN.md) · [docs/](./docs/) ·
[贡献 / 新增客户机](./docs/CONTRIBUTING.md)

---

## 支持的系统

用 `OS=` 选客户机。每个客户机都有自己的指南,讲安装介质、旋钮与注意事项。

| `OS=` | 客户机 | 固件 | 安装 | 状态 | 指南 |
|-------|--------|------|------|------|------|
| `win11`(默认) | Windows 11 企业版 **LTSC 2024** | UEFI + TPM 2.0 | 无人值守(自备 ISO) | 稳定 | [windows.zh-CN.md](./docs/guests/windows.zh-CN.md) |
| `alpine` | Alpine Linux | SeaBIOS | noVNC 控制台安装 | 稳定 | [alpine.zh-CN.md](./docs/guests/alpine.zh-CN.md) |

**新增系统:** 在 `rootfs/build/bin/` 放一个 `start-<os>` 脚本,在 `start-vm` 里加一个分支,再加
`docs/guests/<os>.md` —— 见
[common/engine.zh-CN.md → 新增客户机](./docs/common/engine.zh-CN.md#新增客户机) 与
[CONTRIBUTING.md](./docs/CONTRIBUTING.md)。

---

## 目录

- [架构](#架构)
- [前置要求](#前置要求)
- [快速上手](#快速上手)
- [配置](#配置)
- [网络](#网络)
- [USB 直通](#usb-直通)
- [日志](#日志)
- [访问](#访问)
- [文件布局](#文件布局)
- [排错](#排错)
- [说明与限制](#说明与限制)

---

## 架构

容器的 `CMD` 是 ubase 的 `/start.sh` → supervisord,运行三个服务:

| 服务 | 脚本 | 职责 |
|------|------|------|
| `vm` | `start-vm` | 通用入口:按 `OS` 分派到自包含的每客户机启动器。 |
| `tpm` | `start-tpm` | 独立的 swtpm vTPM 2.0(供需要的客户机用,如 win11);可用 `sctl` 单独控制。 |
| `novnc` | `start-novnc` | 把浏览器控制台(8006 端口)桥接到 QEMU 的 VNC。 |

`start-vm` 让引擎与客户机无关(一个镜像,多种客户机);每个客户机在自己的启动器里负责安装+引导。
深入:[docs/common/engine.zh-CN.md](./docs/common/engine.zh-CN.md)。

---

## 前置要求

- 能访问宿主 **`/dev/kvm`** 的 Docker(`--device=/dev/kvm`);否则 QEMU 退回 TCG 软件模拟,装机太慢。
- **按客户机**准备安装介质 —— 如自备的 Windows 11 LTSC ISO(见该客户机指南)。
- bridge/macvlan 网络需要:`--cap-add NET_ADMIN` 与 `--device=/dev/net/tun`。

---

## 快速上手

默认客户机是 Windows 11(`OS=win11`)。Windows 的完整细节 —— ISO、安装策略、语言、显示 —— 见
[docs/guests/windows.zh-CN.md](./docs/guests/windows.zh-CN.md)。

```bash
cd qemu
mkdir -p images storage
cp /path/to/Win11_LTSC.iso images/win11.iso       # win11 必需 —— 自备 ISO

docker build -t zzci/qemu .

docker run -d --name win11 --device=/dev/kvm \
  -e ZSRV_vm=true -e ZSRV_tpm=true -e ZSRV_novnc=true \
  -e OS=win11 -e WIN11_INSTALL=auto \
  -p 8006:8006 -p 3389:3389 \
  --stop-timeout=180 \
  -v "$PWD/storage:/storage" -v "$PWD/images:/images:ro" \
  zzci/qemu

docker exec win11 tail -f /var/log/supervisord-vm.log   # 观察安装/引导
# 然后打开  http://localhost:8006/
```

> 镜像不烘焙任何默认值,服务**默认关闭**,所以必须显式 `-e ZSRV_vm=true -e ZSRV_tpm=true -e ZSRV_novnc=true`
> 才会启动。`docker stop` 时引擎会请求客户机 ACPI 关机并等待,记得留足时间(`--stop-timeout=180`,或 compose
> 的 `stop_grace_period: 3m`)。在客户机内部关机会**保持关机** —— `sctl start vm` 或重启容器再开。

其它客户机设 `OS=` 并照该客户机指南操作(如 [Alpine](./docs/guests/alpine.zh-CN.md))。仓库附带的
`docker-compose.yml` 含一个 Windows 服务和一段注释掉的 Alpine 示例。

---

## 配置

以下**引擎**旋钮对任何客户机通用,在 `docker run -e ...` 或 compose `environment:` 中设置:

| 变量 | 默认 | 说明 |
|------|------|------|
| `OS` | `win11` | 客户机选择 —— 见[支持的系统](#支持的系统) |
| `RAM_SIZE` | `4G` | 内存 |
| `CPU_CORES` | `2` | 虚拟 CPU 数 |
| `DISK_SIZE` | `128G` | 系统盘大小(仅首装) |
| `NETWORK` | `user` | `user` \| `bridge` \| `macvlan` \| `host` \| `none` |
| `BRIDGE` / `MACVLAN` | – | 这些模式用的网桥名 / 容器网卡 |
| `PORT_FWD` | 按客户机 | `user` 模式宿主→客户机转发,`host-guest` 对(如 `3389-3389,8080-80`) |
| `VGA` | `std` | 显示适配器:`std` \| `virtio` \| `qxl` |
| `RESOLUTION` | – | 强制分辨率,如 `1920x1080`(EDID) |
| `VNC_HOST` | `127.0.0.1` | VNC 监听地址。localhost = 仅经 noVNC 桥接访问;设 `0.0.0.0` 可直接暴露 VNC(如 `--network host` 下) |
| `VNC_PASSWORD` | – | VNC 密码(空 = 无认证)。VNC 协议会截断为前 8 个字符 |
| `USB` | – | USB 直通,`vendor:product` 十六进制(逗号列表) |
| `SERIAL` | – | 宿主串口 → guest COM,TTY 路径(如 `/dev/ttyUSB0`),逗号列表 |
| `CONSOLE` | `off` | `on` 暴露客户机文本控制台(ttyS0/COM1)供 `vm-console` 连接 —— 对 Linux 很有用 |
| `EXTRA_ARGS` | – | 额外原始 QEMU 参数(socket 串口、额外网卡/磁盘、vfio-pci……) |
| `DISK` | `/storage/<os>/…qcow2` | 引导盘路径(指向克隆) |
| `NAME` / `UUID` | `windows` / 自动 | QEMU `-name` / 系统 `-uuid`(UUID 首次生成并存入 conf) |
| `ZSRV_vm` | 关 | 设 `true` 引导客户机;仅构建则留空(再 `win11-installer`) |
| `ZSRV_tpm` | 关 | 设 `true` 运行 vTPM(`tpm` 服务)—— Windows 11 必需 |
| `ZSRV_novnc` | 关 | 设 `true` 运行浏览器控制台桥 |

**客户机专属旋钮**(账号、语言、安装策略、端口转发……)在各客户机指南里:
[Windows 11](./docs/guests/windows.zh-CN.md)、[Alpine](./docs/guests/alpine.zh-CN.md)。首次引导时所选
设置会写入 `storage/<os>/` 下可编辑的每机配置 —— 见
[common/engine.zh-CN.md](./docs/common/engine.zh-CN.md)。

---

## 网络

用 `NETWORK` 选模式;每块网卡在系统里是一个适配器。完整指南(含 USB 与串口直通):
[docs/common/networking-and-devices.zh-CN.md](./docs/common/networking-and-devices.zh-CN.md)。

| 模式 | 行为 | 要求 |
|------|------|------|
| `user` | SLIRP NAT + 端口转发(默认) | 无 |
| `bridge` | 在已有网桥 `$BRIDGE` 上建 tap,加入该二层 | `NET_ADMIN`、`/dev/net/tun`、网桥已存在 |
| `host` | 同 bridge,用于 `--network host` | 同 bridge |
| `macvlan` | 在 `$MACVLAN` 上建 macvtap,guest 获得自己的局域网 IP | `NET_ADMIN`、`--device-cgroup-rule='c *:* rwm'`、一个 macvlan 网络 |
| `none` | 无网卡 | – |

```bash
docker network create -d macvlan \
  --subnet=192.168.1.0/24 --gateway=192.168.1.1 -o parent=enp1s0 lan
docker run -d --name win11 --network lan --device=/dev/kvm --device=/dev/net/tun \
  --cap-add NET_ADMIN --device-cgroup-rule='c *:* rwm' \
  -e NETWORK=macvlan -e MACVLAN=eth0 -v "$PWD/storage:/storage" zzci/qemu
```

> macvlan 注意:**宿主**无法直接访问 macvlan guest IP(同局域网其它机器可以)。

---

## USB 直通

按 `vendor:product`(十六进制)直通宿主 USB 设备,逗号分隔。细节(含串口)见
[docs/common/networking-and-devices.zh-CN.md](./docs/common/networking-and-devices.zh-CN.md)。

```bash
docker run -d --name win11 --device=/dev/kvm --device=/dev/bus/usb \
  -e USB=0bda:8153 -v "$PWD/storage:/storage" zzci/qemu
```

宿主 USB 设备必须在容器内可见(`--device=/dev/bus/usb`,或 privileged)。

---

## 日志

supervisord 把各服务日志写在**容器内**(不在 `/storage` 卷上):

```
/var/log/supervisord-vm.log      # QEMU / 安装 / 引导
/var/log/supervisord-tpm.log     # vTPM
/var/log/supervisord-novnc.log   # 控制台桥
```

```bash
docker exec <container> tail -f /var/log/supervisord-vm.log
```

---

## 访问

- **图形控制台**:`http://localhost:8006/` —— 完整鼠标键盘的 noVNC。
- **文本控制台**(串口,Linux 很好用):用 `CONSOLE=on` 启动,然后接一个终端 ——
  `docker exec -it <container> vm-console`(Ctrl-] 断开)。Linux 客户机需 `console=ttyS0` 才会在此显示登录/启动。
- **远程**:按客户机 —— Windows RDP(客户机 `3389`)、Alpine SSH(客户机 `22`)。`user` 模式下宿主↔客户机映射由 `PORT_FWD` 决定(默认 `3389-3389` / `2222-22`)。

---

## 文件布局

```
qemu/
├── Dockerfile                 # FROM zzci/ubase + qemu/ovmf/swtpm/novnc + 工具
├── docker-compose.yml         # 项目名 "qemu"(windows 服务 + 注释掉的 alpine 示例)
├── README.md / README.zh-CN.md
├── docs/                      # 双语指南(每份 EN + .zh-CN)+ CONTRIBUTING.md
│   ├── common/                # 系统无关:engine.md、networking-and-devices.md
│   └── guests/                # 每系统:windows.md、alpine.md、_template.md
├── images/                    # 本地安装 ISO —— 已 gitignore
├── storage/                   # 持久状态,每客户机一个目录(日志在 /var/log)
│   ├── win11/                 # windows.qcow2、*.conf / *.qemu.conf / *.install、OVMF_VARS、*.tpm/、clones/
│   └── alpine/                # alpine.qcow2(OS=alpine 时)
└── rootfs/build/{bin,services,config}   # 打进镜像
```

---

## 排错

| 现象 | 原因 / 处理 |
|------|-------------|
| 裸 `docker run` 什么都不启动 | 服务默认关闭 —— 加 `-e ZSRV_vm=true -e ZSRV_tpm=true -e ZSRV_novnc=true`。 |
| `sctl list` 里 `vm` 服务反复重启 | 看 `/var/log/supervisord-vm.log`;多为某个 QEMU 参数或上一个容器遗留的 qcow2 锁。 |
| 黑屏 / `/dev/kvm` 告警 | 没带 `--device=/dev/kvm` → TCG;加上该设备。 |
| `macvtap … Operation not permitted` | 加 `--device-cgroup-rule='c *:* rwm'`(及 `--cap-add NET_ADMIN`)。 |
| `NETWORK=bridge needs BRIDGE=…` | 设 `BRIDGE` 为已有网桥并加 `--device=/dev/net/tun`。 |

客户机专属问题(安装循环、语言、显示)在各客户机指南里。

---

## 说明与限制

- 系统在挂载的 `storage/<os>/…qcow2` 里,不在镜像中(镜像约 750 MB)。
- 安装介质按客户机准备,授权要求的需自备(如 Windows ISO 从不自动下载)。
- 仅供虚拟化/实验用途;请遵守各系统授权条款。
