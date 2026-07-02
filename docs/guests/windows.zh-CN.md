# Windows 11 客户机(`VMD_OS=win11`)

[English](./windows.md) · **中文**

全自动 **Windows 11** 安装与运行:提供一个 ISO,首次启动自动安装(KVM 下约 13 分钟),之后每次
启动直接进系统。UEFI(OVMF)+ vTPM 2.0 + virtio 磁盘/网卡(驱动已滑流注入),RDP 默认开启,ACPI
关机确定可靠。

## 前提

- `images/win11.iso` 挂载到 **`/images/win11.iso`** —— 自备的 Windows 11 ISO(推荐 LTSC;绝不
  自动下载)。
- `/images/virtio-win.iso` 可选 —— 缺省时自动从 Fedora 下载。
- `--device=/dev/kvm`。

## 配置

```toml
[guest.win11]
dir       = "/vms/win11"
disk      = "windows.qcow2"
disk_size = "128G"              # 稀疏文件,按需增长
ram       = "4G"
cpus      = 2
launch    = "win11"
tpm       = true
seed      = [ { template = "/usr/share/OVMF/OVMF_VARS_4M.fd", to = "{state}.OVMF_VARS.fd" } ]

[guest.win11.install]
policy      = "auto"            # auto | force | none
source_iso  = "/images/win11.iso"
virtio_iso  = "/images/virtio-win.iso"
username    = "docker"          # unattend 创建的本地管理员
password    = "admin"
language    = "zh-CN"           # UI 语言;须存在于 ISO(自动校验,不在则回退默认)
region      = "zh-CN"           # 用户区域(可选;默认同 language)
keyboard    = "zh-CN"           # 输入法区域(可选;默认同 language)
image_index = 1                 # install.wim 版本索引(LTSC ISO 上 1 = LTSC)
# virtio_sha256 = "<hex>"       # 校验自动下载的 virtio-win.iso(不设置则不校验)
```

install 的键会大写后作为环境变量传给 `{dir}/scripts/install`;`language` 也接受友好名
(`Chinese` → `zh-CN`)。装简体中文系统:三个 locale 都设 `zh-CN` 并使用 zh-CN 的 ISO。

## 安装脚本做了什么

`{dir}/scripts/install`(你的可编辑副本)拥有完整流水线:

1. 解包源 ISO;用 `install.wim` 校验 `image_index` 与 `language`(语言不存在时回退镜像默认,
   避免安装程序停在语言页);
2. 渲染 `autounattend.xml`(账户、区域;跳过 TPM/SecureBoot/内存检查;开 RDP;精简遥测;
   **禁用休眠 + 电源键=关机**,保证 ACPI 控制可靠);
3. 滑流注入 virtio 驱动(`$WinpeDriver$`);重打无按键提示的 UEFI ISO(免"Press any key");
4. 用一次性精简安装 VM(`cache=unsafe`、无 TPM/网卡)跑安装,等它干净关机;
5. 写 `{state}.install` = `installed`;vmd 随即用正式设备模型引导系统。

安装期间:Web 控制台可实时观看;qcow2 持续增长;vmd 日志最终打印 `install finished in Ns`。

## 访问

- **Web 控制台** —— `http://<host>:8006`(noVNC 显示 + 电源控制)。
- **RDP** —— launcher 转发 3389,unattend 已启用(`docker run -p 127.0.0.1:3389:3389`,用
  `username`/`password` 登录)。

## 克隆

```bash
docker exec <container> win11-clone <name>          # 链接克隆(写时复制,秒级)
docker exec <container> win11-clone <name> --full   # 完整独立副本
```

然后加一个 `[guest.<name>]` 块把 `disk` 指向克隆盘即可运行(独立容器 `VMD_OS=<name>`,或切换
当前容器)。克隆与源共享 SID/主机名——实验环境无妨。

## 排障

- **安装停在语言选择页** —— ISO 不含配置的 `language`;校验器通常会自动回退(看安装日志)。
- **重装** —— `policy = "force"`(一次性抹盘),或删除 `{state}.install*` 与磁盘。
- **分辨率** —— 编辑 `{dir}/scripts/launcher` 里的 `xres/yres`(默认 1920×1080)。
- **安装很慢** —— 看日志是否有 TCG 警告;需要可用的 `/dev/kvm`。
