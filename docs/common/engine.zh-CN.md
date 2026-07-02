# 引擎(系统无关)

[English](./engine.md) · **中文**

`zzci/qemu` 是一个运行多种客户机的 Docker 镜像。本页讲**对每种客户机都一样**的部分 —— 分派、服务、控制台、
每机配置/状态约定、存储布局,以及**如何新增客户机**。各系统专属内容在 [../guests/](../guests/);
网络与设备直通见 [networking-and-devices.zh-CN.md](./networking-and-devices.zh-CN.md);
概览见[根 README](../../README.zh-CN.md)。

---

## 分派 —— 一个镜像,多种客户机

容器的 `CMD`(来自 `zzci/ubase`)是 `/start.sh` → supervisord,运行三个服务:

| 服务 | 脚本 | 职责 |
|------|------|------|
| `vm` | `start-vm` | 读 `$OS` 并 `exec` 对应启动器(`win11`→`start-win11`,`alpine`→`start-alpine`)。 |
| `tpm` | `start-tpm` | 前台运行 `swtpm`(vTPM 2.0)供需要的客户机用;由 `ZSRV_tpm` 开关。 |
| `novnc` | `start-novnc` | `websockify` 把 `8006` 端口的浏览器控制台桥接到 QEMU 的 VNC(`:0` / `5900`)。 |

服务用 ubase `ZSRV_*` 环境变量开关(`ZSRV_vm`、`ZSRV_tpm`、`ZSRV_novnc`),运行时用
`sctl start|stop|restart <name>` 控制。**镜像不烘焙任何默认值**,所以每个服务**默认关闭、必须显式启用** ——
裸 `docker run` 什么都不启动。按需启用,如 `-e ZSRV_novnc=true -e ZSRV_tpm=true -e ZSRV_vm=true`;
仅构建的容器把 `ZSRV_vm` 留空(再 `docker exec <c> win11-installer`)。

`start-vm` 保持精简 —— 它集中**OS 无关的引擎默认值**(内存、CPU、磁盘、machine、网络、显示;都可用
`docker run -e …` 覆盖)并导出,然后分派。客户机专属配置(如 Windows 的账户/区域/安装策略)留在该客户机自己的
启动器里,不放这里。每个客户机的启动器**各自负责安装+引导**(所以互不影响),只从 **`qemu-common.sh`** 取通用的
共享辅助(日志、KVM/固件探测、VNC、端口转发拼接、优雅关机托管)—— 不共享任何客户机逻辑。

**虚拟机生命周期。** `docker stop` 时启动器先请求客户机 ACPI 关机,并**等待** QEMU 刷盘完成后才退出
(不会在写盘途中被 SIGKILL),磁盘保持干净 —— 请给足 `stop_grace_period`。在客户机**内部**关机会让它
关闭并**保持关机**(`vm` 程序以 0 退出;supervisord 的 `autorestart=unexpected` 只会重启*崩溃*)——
用 `sctl start vm` 或重启容器再次引导。客户机**重启**则只是原地重置虚拟机。

---

## 控制台、KVM、日志

- **控制台**:打开 `http://<host>:8006/` 即 noVNC 浏览器控制台(完整鼠标键盘)。每种客户机都在 QEMU
  VNC `:0` 暴露显示,由 `novnc` 桥接。
- **KVM**:传 `--device=/dev/kvm`。每个启动器都会探测它并用 `-cpu host` + `accel=kvm`;没有则回退
  `accel=tcg` + `-cpu max`(软件模拟 —— 装机太慢)。会打印告警。
- **日志**:supervisord 写在**容器内**(不在 `/storage` 卷上):`/var/log/supervisord-vm.log`
  (QEMU / 安装 / 引导)、`/var/log/supervisord-tpm.log`(vTPM)、`/var/log/supervisord-novnc.log`
  (控制台桥)。用 `docker exec <c> tail -f /var/log/supervisord-vm.log` 查看。

---

## vTPM(`tpm` 服务)

需要 TPM 2.0 的客户机(如 Windows 11)由 **`swtpm`** 提供,它作为独立的 supervisord 程序(`start-tpm`)
运行,而非内联启动 —— 这样可单独控制(`sctl start|stop|restart tpm`)并被任意客户机复用。与其他服务一样,它由
ubase 的 `ZSRV_*` 开关控制:**`ZSRV_tpm`**(和所有服务一样默认关 —— 需要 TPM 的客户机如 Windows 11
设 `ZSRV_tpm=true`)。没有单独的 TPM 环境变量。

状态存在**客户机磁盘旁**(`<disk>.tpm/`,控制套接字 `<disk>.tpm/swtpm-sock`),与其他每机状态一样按磁盘路径
派生 —— 因此跨重启持久、每个克隆相互隔离。`tpm` 服务以更低的 supervisord 优先级先于 `vm` 启动;由于程序并行
启动,客户机启动器会在引导 QEMU 前**等待** vTPM 套接字就绪。它不含任何 swtpm 细节,只做委托:`start-tpm socket`
返回控制套接字路径(供 `-readconfig`),`start-tpm wait` 必要时拉起服务并阻塞到就绪,超时则快速报错,而不是让
Windows 在无 TPM 的情况下引导。新增需要 TPM 的客户机启用 `ZSRV_tpm` 并调用 `start-tpm wait`;不需要 TPM
的客户机保持其关闭即可。

---

## 每机配置与状态约定

每种客户机都按**磁盘路径**派生各自的每机文件:取 `STATE="${DISK%.*}"`,磁盘
`/storage/win11/windows.qcow2` 会派生出 `windows.conf`、`windows.qemu.conf`、`windows.OVMF_VARS.fd`、
`windows.tpm/`、`windows.monitor.sock` 等 —— 都在磁盘旁边。把 `DISK` 指向另一路径(如克隆)即自动获得
独立状态;多台 VM 共用一个 `/storage` 也不冲突。

配置文件是 **incus 风格**:首次引导时启动器用环境变量生成 `<disk>.conf`,此后以它为准。**编辑后重启**
即可改运行设置;删除它则从环境变量重新生成。安装时烧入的身份/语言(账号、语言、版本)**不**从 conf 重读。

客户机可能在磁盘旁保留的状态文件:

| 文件 | 含义 |
|------|------|
| `<disk>.conf` | 可编辑的每机配置(首启后即权威) |
| `<disk>.qemu.conf` | 生成的 `-readconfig` 拓扑(每次引导重建;勿改) |
| `<disk>.install` | 安装生命周期记录(`installing` / `installed`)—— 把关是否重装 |
| `<disk>.force-applied` | `WIN11_INSTALL=force` 的一次性保护标记 |
| `<disk>.OVMF_VARS.fd`、`<disk>.tpm/` | UEFI NVRAM 与 swtpm 状态(固件类客户机) |

---

## 存储布局

```
storage/
├── <os>/        # 每客户机状态:qcow2 + 所有 <disk>.* 文件 + clones/ + base.qcow2
├── logs/        # vm.log、novnc.log(共享)
└── …
```

每客户机一个目录(`storage/win11/`、`storage/alpine/`)。临时构建产物(如重制的 Windows 安装 ISO)放
`/tmp`,**从不**进 `/storage`。输入 ISO 只读挂载到 `/images`。

---

## 新增客户机

引擎、控制台、网络与设备直通都与客户机无关,所以新增一个客户机很小 —— 它自己负责引导逻辑,通用辅助则共享自
`qemu-common.sh`:

1. **写 `rootfs/build/bin/start-<os>`** —— 一个脚本,设好 `LOG_TAG` 后 source `qemu-common.sh`,然后:
   - 从环境变量读取旋钮(`RAM_SIZE`、`CPU_CORES`、`DISK`、`NETWORK`……);
   - `DISK` 默认 `$STORAGE/<os>/<name>.qcow2`,并 `mkdir -p "$(dirname "$DISK")"`(沿用每 OS 目录约定);
   - 用共享的 `detect_kvm` 探测 KVM,用 `build_vnc_args` 配 VNC,拼装自己的 QEMU 命令,(可选)在 `/images`
     或 `/storage` 下定位/下载安装介质;
   - 后台启动 QEMU,把 pid 交给 `supervise_qemu`(优雅关机 + 保持关机);
   - 临时构建产物放 `/tmp`,不放 `/storage`。
2. **接入 `start-vm`** —— 加一个 `case` 分支,把你的 `OS` 取值映射到 `start-<os>`。
3. **复用通用部分** —— 网络与设备直通用法相同,见
   [networking-and-devices.zh-CN.md](./networking-and-devices.zh-CN.md)。若客户机无人值守安装,
   可像 Windows 那样用安装状态文件(`<disk>.install`)。
4. **写文档** —— 新增 `docs/guests/<os>.md`(+ `.zh-CN.md`),并在 [docs/README.zh-CN.md](../README.zh-CN.md)
   里列出。

[Alpine](../guests/alpine.zh-CN.md) 是最简参考(控制台安装、SeaBIOS、无无人值守流程);
[Windows](../guests/windows.zh-CN.md) 是完整示例(无人值守安装、TPM/UEFI、注入驱动、安装状态)。
新客户机的文档可从 [`docs/guests/_template.zh-CN.md`](../guests/_template.zh-CN.md) 起步。

### 启动器骨架

一个可复制的最简 `rootfs/build/bin/start-<os>`:

```bash
#!/usr/bin/env bash
# start-<os> —— 最简客户机启动器。通用辅助共享自 qemu-common.sh;把 OS=<os> 接入 start-vm。
set -euo pipefail
LOG_TAG=<os>
source /build/bin/qemu-common.sh
: "${STORAGE:=/storage}"; : "${IMAGES:=/images}"
: "${RAM_SIZE:=2G}"; : "${CPU_CORES:=2}"; : "${DISK_SIZE:=16G}"; : "${MACHINE:=q35}"
: "${DISK:=$STORAGE/<os>/<os>.qcow2}"          # 每 OS 目录约定
: "${NETWORK:=user}"; : "${PORT_FWD:=2222-22}"; : "${EXTRA_ARGS:=}"
: "${VNC_HOST:=127.0.0.1}"; : "${VNC_PASSWORD:=}"
STATE="${DISK%.*}"; MONITOR="$STATE.monitor.sock"; VNC_SECRET="/tmp/$(basename "$STATE").vncpw"

mkdir -p "$(dirname "$DISK")"
detect_kvm; ACCEL=(-machine "${MACHINE},accel=$ACCEL_MODE" -cpu "$CPU_MODEL")   # 来自 qemu-common.sh
build_vnc_args                                                                  # -> VNC_ARGS
[ -f "$DISK" ] || qemu-img create -f qcow2 "$DISK" "$DISK_SIZE" >/dev/null
# TODO: 在 /images 或 /storage 下定位/下载安装介质;临时产物放 /tmp。
rm -f "$MONITOR"

# shellcheck disable=SC2086
qemu-system-x86_64 "${ACCEL[@]}" -smp "$CPU_CORES" -m "$RAM_SIZE" \
    -device virtio-rng-pci -device VGA "${VNC_ARGS[@]}" \
    -monitor "unix:$MONITOR,server,nowait" -name "<os>" \
    -netdev "user,id=net0$(user_hostfwd "$PORT_FWD")" -device virtio-net-pci,netdev=net0 $EXTRA_ARGS \
    -drive "file=$DISK,if=none,id=disk0,format=qcow2,cache=writeback" \
    -device virtio-blk-pci,drive=disk0,bootindex=1 &
supervise_qemu $!   # 优雅关机 + 保持关机(来自 qemu-common.sh)
```

再给 `start-vm` 加一个 `case` 分支:

```bash
    <os>|<alias>)
        log "OS=$OS -> start-<os>"; exec /build/bin/start-<os> ;;
```
