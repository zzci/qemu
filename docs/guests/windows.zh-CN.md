# Windows 11 客户机(`OS=win11`)

[English](./windows.md) · **中文**

在 `zzci/qemu` 引擎上运行 **Windows 11 企业版 LTSC**:全程无人值守安装(无需点击)、slipstream 注入
virtio 驱动、TPM 2.0 + UEFI,装完直接从磁盘引导。

> **为什么用 LTSC?** Windows 11 IoT 企业版 LTSC 不含商店 / Copilot / 消费应用 / Teams / 小组件,
> 本就接近 "tiny11",更稳定、维护周期更长。

引擎概览与快速上手见[根 README](../../README.zh-CN.md);本页是 Windows 的深入指南。

---

## 工作原理

两个脚本负责 Windows 的完整生命周期(共享 `qemu-common.sh` 的辅助):

| 脚本 | 阶段 | 设备模型 |
|------|------|----------|
| `win11-installer` | 一次性安装 | 精简且一次性:OVMF + virtio-blk + AHCI(光盘) + rng + 普通 VGA,**无 TPM/网卡/balloon**,`cache=unsafe` 提速。在 `/tmp/win11-build` 重制 ISO(注入 virtio + `autounattend.xml`),装入 `windows.qcow2`,记录结果,完成后关机。 |
| `start-win11` | 每次引导 | 由生成的 `-readconfig` 文件驱动的**完整**运行模型:TPM 2.0、virtio-blk/net、balloon、rng、USB(xhci + tablet)、显示。 |

`start-win11` 把静态机器拓扑渲染到 `/storage/win11/windows.qemu.conf`,用 `-readconfig` 启动 QEMU;
只有动态 / 无 config 组的参数(`-cpu`、显示、monitor、身份、网络、USB、数据盘)留在命令行上。

**TPM 2.0** 本身是 `swtpm`,作为独立的 **`tpm` 服务**运行(不再由 `start-win11` 内联启动),因此可单独控制
(`sctl start|stop|restart tpm`)并被其他客户机复用。`start-win11` 会在引导前等待 vTPM 套接字,若始终不就绪
则快速报错 —— 见 [common/engine.md → vTPM](../common/engine.zh-CN.md#vtpmtpm-服务)。装机阶段**不带** TPM
(`autounattend.xml` 绕过检查),vTPM 只在运行时挂载。

---

## 提供 ISO

Windows ISO **从不自动下载**(微软评估版链接会过期)。请自行挂载到 `/images/win11.iso`,
或用 `SOURCE_ISO` 指向任意路径(`-e SOURCE_ISO=/path/to.iso`):

```yaml
volumes:
  - ./images/Win11_LTSC_zh-cn.iso:/images/win11.iso:ro   # 你的 ISO,映射成 win11.iso
  - ./images/virtio-win.iso:/images/virtio-win.iso:ro    # virtio 驱动,否则自动下载
```

重制前,安装器用 `wiminfo` 检查 ISO:

- `IMAGE_INDEX` 越界则**直接报错**(并打印有效范围 `1..N`);
- 若 `LANGUAGE` 不在镜像中,会**告警并回退**到镜像默认语言,使安装继续完成,而不是卡在语言选择页。

---

## 安装策略与状态 —— `WIN11_INSTALL`

| 取值 | 行为 |
|------|------|
| `auto` | 无已完成安装记录则安装;也会恢复被中断的安装。 |
| `force` | 擦除并重装**一次**(有标记保护:客户机重启不会再次擦除)。 |
| `none`(默认) | 从不自动安装;无已完成安装时等待手动 `win11-installer`。 |

判定由一个**安装状态文件** `/storage/win11/windows.install` 把关,其首行为 `installing` 或 `installed`:

- `win11-installer` 开始时写 `installing`,只有安装完成才写 `installed` —— 这样写了一半的盘**不会**被
  误当成可用系统。
- `auto` 只要状态不是 `installed` 就安装(全新**或**中断);引导守卫在状态变成 `installed` 前拒绝引导。
- 早于此文件的旧盘(无记录)在首次接触时被**迁移**为 `installed`,使已有系统继续引导。
- `force` 重装一次并写 `windows.force-applied`;删除该标记(或运行 `FORCE=1 win11-installer`)可再次强制。

```bash
docker exec <container> win11-installer        # 手动安装,已记录 installed 则跳过
FORCE=1 docker exec <container> win11-installer # 从零重建
```

---

## 语言与版本

`LANGUAGE` / `REGION` / `KEYBOARD` 接受友好名称或 `xx-XX` 代码;默认均为 `en-US`。

```yaml
environment:
  LANGUAGE: "Chinese"   # 或 zh-CN
  REGION: "zh-CN"
  KEYBOARD: "zh-CN"
```

> 所选 `LANGUAGE` 必须存在于源 ISO 中。评估版 ISO 通常**仅含 en-US**;其它语言请用多语言 LTSC ISO。

`IMAGE_INDEX`(默认 `1`)选择 `install.wim` 中的版本;评估版 ISO 上 `1` 即 LTSC。

---

## 账号与 RDP

`USERNAME`(默认 `docker`)/ `PASSWORD`(默认 `admin`)在安装时烧入,也是 RDP 凭据。默认 `user` 网络模式下,
RDP 由 `PORT_FWD` 转发(默认 `3389-3389`,即宿主 `3389` → 客户机 `3389`);`bridge`/`macvlan` 模式下客户机有
自己的局域网 IP,直接 RDP 即可。详见 [networking-and-devices.zh-CN.md](../common/networking-and-devices.zh-CN.md)。

---

## 显示:`std` 装机,`virtio` 运行

安装器始终用**普通 VGA(`std`)**—— 对 Windows Setup 最简单、永远能用。virtio-GPU 驱动(`viogpudo`)在安装时
已 slipstream 注入,所以**装完后即可把运行时显示切到 `virtio`**,获得宽屏分辨率与 noVNC 自适应。

```yaml
environment:
  VGA: "virtio"            # std | virtio | qxl
  RESOLUTION: "1920x1080"  # 经 EDID 强制,配 VGA=virtio 最佳
```

- `std` 以 1024×768 启动,仅几种 4:3 模式。
- `virtio` + `RESOLUTION` 给出强制宽屏(在已装系统上,桌面加载几秒后生效)。
- 桌面加载后控制台可能**黑屏** —— 这是显示器休眠而非卡死;按一下键即可唤醒(RDP/`3389` 可达即证明系统在运行)。

> 一块**没有** virtio-GPU 驱动的外部磁盘在 `VGA=virtio` 下可能卡在 Windows Boot Manager。请用本引擎安装
> (驱动已注入),或改回 `VGA=std`。

---

## 存储与每机配置

所有 Windows 状态都在 `storage/win11/` 下(每客户机一个目录);重制出的安装 ISO 是临时产物,
构建于 `/tmp/win11-build`,从不落在 `/storage`。

```
storage/win11/
├── windows.qcow2          # 系统盘
├── windows.conf           # 可编辑的每机配置(首启后即权威)
├── windows.qemu.conf      # 生成的 -readconfig 拓扑(勿改,每次引导重建)
├── windows.install        # 安装状态记录(installing | installed)
├── windows.OVMF_VARS.fd   # UEFI NVRAM
└── windows.tpm/           # swtpm 状态
```

首次引导时 `start-win11` 用环境变量生成 `windows.conf`,此后以它为准 ——**编辑后重启**即可改资源/网络/显示:

```ini
# /storage/win11/windows.conf
NAME=windows            # 虚拟机名(-name)
UUID=…                  # 系统 UUID(-uuid),首次生成后固定
CPU_CORES=4
RAM_SIZE=8G
MACHINE=q35
DISK=/storage/win11/windows.qcow2
NETWORK=user
PORT_FWD=3389-3389      # user 模式宿主-客户机转发(如 3389-3389,8080-80)
VGA=virtio
RESOLUTION=1920x1080
USB=
SERIAL=                 # 宿主串口 -> guest COM,TTY 路径(如 /dev/ttyUSB0,/dev/ttyS0)
EXTRA_ARGS=
```

稳定的 `UUID` 只生成一次并保存在 conf 中,使客户机硬件标识(授权/激活)跨重启一致;每个克隆有自己的 UUID。
要从环境变量重新生成 conf,删除该文件再重启即可。

---

## 克隆

```bash
docker exec <container> win11-clone dev          # 链接克隆(在密封基底上写时复制)
docker exec <container> win11-clone dev --full   # 独立完整副本
```

每块盘按磁盘路径派生各自的固件/TPM/monitor/配置/安装状态,所以多个克隆可在同一 `/storage` 上并行运行。
链接克隆共享只读的 `storage/win11/base.qcow2`。克隆继承源的机器 SID/主机名 —— 若需唯一身份,克隆前在
Windows 内运行 `sysprep /generalize`。

```bash
docker run -d --name win11-dev --device=/dev/kvm -e WIN11_INSTALL=none \
  -e DISK=/storage/win11/clones/dev.qcow2 \
  -p 8016:8006 -v "$PWD/storage:/storage" zzci/qemu
```

---

## 排错

| 现象 | 原因 / 处理 |
|------|-------------|
| 安装循环到"计算机意外重启" | `autounattend.xml` 无效;看 `/var/log/supervisord-vm.log`,或挂载 qcow2 读 `C:\Windows\Panther\setuperr.log`。 |
| 卡在语言选择页 | `LANGUAGE` 不在 ISO 中 —— 安装器告警并回退到 ISO 默认语言;请用多语言 LTSC ISO。 |
| 容器一直等待、提示 "no completed install" | 状态不是 `installed`(全新或中断)—— 运行 `win11-installer`,或设 `WIN11_INSTALL=auto`。 |
| 切 `VGA` 后卡在 "Windows Boot Manager" | 该盘缺少新显示驱动 —— 改回 `VGA=std`,或用目标适配器重装。 |
| 控制台黑屏但 RDP 正常 | 显示器休眠而非卡死 —— 按键唤醒。 |
| `/dev/kvm` 告警或很慢 | 启动时没带 `--device=/dev/kvm` → TCG;加上该设备。 |
