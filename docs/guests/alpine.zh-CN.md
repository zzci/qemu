# Alpine Linux 客户机(`VMD_OS=alpine`)

[English](./alpine.md) · **中文**

**零介质**自装的极简 Linux 客户机:安装脚本自动下载官方 Alpine 云镜像(tiny-cloud,
BIOS/SeaBIOS)并做成客户机磁盘,数秒即进入串口控制台。

## 配置

```toml
[guest.alpine]
dir       = "/vms/alpine"
disk      = "alpine.qcow2"
disk_size = "8G"                # 云镜像会被扩容到该大小
ram       = "2G"
cpus      = 2
launch    = "alpine"

[guest.alpine.install]
policy = "auto"
# url           = "https://…/custom.qcow2"   # 指定镜像(跳过镜像站解析)
# alpine_mirror = "https://mirror…/alpine/latest-stable/releases/cloud"
# sha512        = "<hex>"                     # 固定镜像校验值(可选)
```

安装脚本从镜像站索引解析最新的 `generic_alpine-*-x86_64-bios-tiny-r*.qcow2`,`qemu-img convert`
到 `VMD_DISK` 并扩容到 `VMD_DISK_SIZE`。下载会按 `sha512` 固定值(或镜像站发布的
`<file>.sha512`)校验——不匹配即中止,仅在拿不到校验值时告警放行。`FORCE=1`
(`policy = "force"`)重新下载重建。

## 访问

- **串口控制台** —— 主要入口:Web 控制台 → 打开控制台(`/console`,COM1,`console=ttyS0`)。
- **VNC** —— 也有普通 VGA 文本画面。
- **SSH** —— 模板转发宿主 2222 → 客户机 22(`docker run -p 127.0.0.1:2222:2222`);需先在
  客户机内启用 sshd。

云镜像使用 tiny-cloud:无数据源首次启动时,串口上 `root` 无密码可登录(请立即设置密码;也可
通过额外光驱挂 nocloud seed ISO 走 cloud-init 初始化)。

## 说明

- SeaBIOS,无 TPM/UEFI —— launcher 是最简模板,适合作为新客户机的起点。
- 后续扩容:`qemu-img resize {dir}/alpine.qcow2 +8G`,再在客户机内扩分区。
