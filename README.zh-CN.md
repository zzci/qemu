# qemu —— Docker 里的 QEMU/KVM 客户机

[English](./README.md) · **中文**

一个打包成单个 Docker 镜像(`zzci/qemu`)、可在浏览器里操作的小型 **QEMU/KVM 虚拟化引擎**。核心是
一个静态 Rust 二进制 **`vmd`**,通过 QMP 守护 QEMU,**不含任何针对特定系统的逻辑**——客户机就是纯
配置:`vmd.toml` 里的一个 `[guest.<name>]` 块加一个模板脚本文件夹。基于
[`zzci/ubase`](https://hub.docker.com/r/zzci/ubase)(Ubuntu 22.04 + tini + supervisord)。

特性:内嵌 Web 控制台(noVNC + 串口终端,中英文)· KVM 加速 · 无人值守安装(Windows 11、Alpine)·
vTPM 2.0 · VNC/串口走 unix socket(除 Web 端口外零 TCP 监听)· 电源 API 与确定性 ACPI 关机 ·
可选访问密码 · 每客户机独立持久化主目录 + 用户可编辑脚本。

📚 **指南:**[引擎](./docs/common/engine.zh-CN.md) ·
[网络、串口、USB 与设备](./docs/common/networking-and-devices.zh-CN.md) ·
[Windows](./docs/guests/windows.zh-CN.md) · [Alpine](./docs/guests/alpine.zh-CN.md) ·
[贡献 / 新增客户机](./docs/CONTRIBUTING.md)

---

## 前提

- Docker 可访问宿主机 **`/dev/kvm`**(`--device=/dev/kvm`);否则 QEMU 退回 TCG 纯软件模拟,慢到
  不可用。
- 按客户机需要提供安装介质——例如自备的 Windows 11 ISO。Alpine 无需介质,自动拉取官方云镜像
  (只需网络)。
- 桥接/tap 网络需要 `--cap-add NET_ADMIN` 和 `--device=/dev/net/tun`。

## 快速开始

```bash
docker run -d --name qemu --device=/dev/kvm \
  -e ZSRV_vmd=true -e VMD_OS=win11 \
  -v "$PWD/vms:/vms" -v "$PWD/images:/images:ro" \
  -p 127.0.0.1:8006:8006 -p 127.0.0.1:3389:3389 zzci/qemu
```

- 把 Windows ISO 放到 `images/win11.iso`(`images/virtio-win.iso` 可选,缺省自动下载)。首次启动
  全自动安装(KVM 下约 13 分钟),装完直接引导系统。打开 **http://localhost:8006** 观看与控制。
- 换 `VMD_OS=alpine` 则启动 Alpine——无需介质,安装脚本自动获取官方云镜像。

也可以用 [docker-compose.yml](./docker-compose.yml)。

## 安全

默认值面向单机实验环境,对外暴露前请先检查:

- **未开启鉴权前保持只绑定 localhost(即上面的默认写法)。** 不设置 `[web] password` 时,任何能访问
  8006 端口的人都拥有完整的 VNC 与电源控制;示例的 guest 账号(`docker`/`admin`)是公开的——请修改,
  并让 3389 仅限本机或置于防火墙之后。
- **远程访问请加 TLS。** vmd 只提供明文 HTTP;请置于 HTTPS 反向代理之后(带
  `X-Forwarded-Proto: https` 时登录 cookie 自动加 `Secure`)。密码连续错误会被逐步延迟,但传输
  加密由代理负责。
- **固定下载校验值。** Alpine 安装会用镜像站的 `.sha512`(或 install 配置里的 `sha512 = "…"`)校验云镜像;
  Windows 安装只有设置 `virtio_sha256 = "…"` 时才校验自动下载的 `virtio-win.iso`。

## 配置

一切都在 **`vmd.toml`**。查找顺序:`$VMD_CONFIG` → `/vms/vmd.toml` → `/etc/vmd/vmd.toml`
(镜像内置默认,首次运行自动复制到 `/vms/vmd.toml` 供编辑)。用 `VMD_OS` 或文件里的 `default`
选择客户机。

```toml
default = "win11"

[web]
port = 8006
# password = "change-me"        # Web 控制台访问密码(留空/缺省 = 不鉴权)

[guest.win11]
dir       = "/vms/win11"        # 客户机主目录:磁盘、状态、日志、scripts/ 都在这里
disk      = "windows.qcow2"     # 相对 dir
disk_size = "128G"
ram       = "4G"
cpus      = 2
launch    = "win11"             # 模板文件夹,首启复制到 {dir}/scripts/(编辑副本即可定制)
tpm       = true                # 内置 vTPM 2.0(受管 swtpm)
seed      = [ { template = "/usr/share/OVMF/OVMF_VARS_4M.fd", to = "{state}.OVMF_VARS.fd" } ]

[guest.win11.install]           # 首启安装,由 policy 把关
policy      = "auto"            # auto | force | none
source_iso  = "/images/win11.iso"
username    = "docker"
password    = "admin"
language    = "zh-CN"           # UI 语言,须存在于 ISO 中(自动校验回退)
image_index = 1
```

install 里的每个键都会**大写后作为环境变量**传给安装脚本;磁盘与大小从父级以
`VMD_DISK`/`VMD_DISK_SIZE` 传入。详见 [docs/common/engine.zh-CN.md](./docs/common/engine.zh-CN.md)。

## Web 控制台

一个端口(8006)提供全部功能:首页(实时状态、VM 信息、端口转发)、开机/关机/重启/强制关闭、
noVNC 显示、xterm 串口终端,以及 JSON API(`/status`、`/info`、
`POST /power/<start|shutdown|reset|poweroff>`)。界面中英文可切换。设置 `[web] password` 后需先
登录。

容器内命令行:

```bash
vmd power status|start|shutdown|reset|poweroff
vmd print          # 干跑:显示解析后的计划与 QEMU 命令
```

## 定制客户机

首次启动时模板脚本被复制到 `{dir}/scripts/`(如 `vms/win11/scripts/launcher`、`.../install`)。
**直接编辑这些副本**——它们属于你,永不被覆盖。launcher 用 `VMD_*` 环境变量拼装 QEMU 命令;改
分辨率、加磁盘、网卡、串口、USB 都在这里。参见
[网络与设备](./docs/common/networking-and-devices.zh-CN.md)。

## 文件布局(每客户机,位于 `dir` 下)

```
vms/win11/
├── windows.qcow2            # 磁盘
├── windows.OVMF_VARS.fd     # UEFI NVRAM
├── windows.tpm/             # vTPM 状态
├── windows.{qmp,vnc,console}.sock
├── windows.install(ed)      # 安装标记
├── windows.uuid             # 稳定的 SMBIOS UUID
├── qemu.log                 # QEMU 自身输出
└── scripts/                 # 可编辑的 launcher + install
```

## 排障

- **很慢 / 日志有 TCG 警告** —— 容器内没有可用的 `/dev/kvm`。
- **安装像卡住了** —— 用 Web 控制台实时观看;装完安装 VM 会自动关机。
  `vms/<g>/<disk>.install` 记录 `installing`/`installed`。
- **重装** —— `policy = "force"`(一次性抹盘),或删除磁盘与标记文件。
- **日志** —— `docker exec <c> tail -f /var/log/supervisord-vmd.log`;QEMU 自身 stderr 在
  `{dir}/qemu.log`。
