# Alpine Linux 客户机(`OS=alpine`)

[English](./alpine.md) · **中文**

一个小巧的、控制台安装的 Linux 客户机。它在**没有**无人值守流程的情况下端到端验证 `zzci/qemu` 引擎 ——
你在浏览器控制台里交互式安装 Alpine。既可作为 KVM/VNC/网络是否正常的快速自检,也可当作一台小型 Linux 虚拟机。

引擎概览见[根 README](../../README.zh-CN.md)。

---

## 工作原理

`start-alpine` 引导 Alpine ISO 并附带一块空盘,暴露 VNC。与 Windows 不同,它**无需 OVMF/TPM**
(SeaBIOS 足够)、**不重制介质** —— 整个安装由控制台驱动。空盘没有引导扇区,所以 `bootindex=1` 会落到
ISO(`bootindex=2`)进入 live 安装器;装好后磁盘优先引导。

ISO 解析顺序为 `ALPINE_ISO` → `/images/alpine.iso` → `/storage/alpine.iso`,否则下载
(`alpine-virt`,体积小、CDN 稳定)。状态保存在 `storage/alpine/` 下。

---

## 安装

```bash
docker run -d --name alpine --device=/dev/kvm \
  -e ZSRV_vm=true -e ZSRV_novnc=true -e OS=alpine \
  -v "$PWD/storage-alpine:/storage" -p 8006:8006 -p 2222:2222 zzci/qemu
```

然后打开浏览器控制台,交互式安装:

```
打开  http://localhost:8006/

登录: root            # 无密码
setup-alpine           # 交互式配置(键盘、主机名、网络、磁盘……)
setup-disk -m sys /dev/vda    # 安装到磁盘(sys 模式)
poweroff               # 关机后容器重启,从磁盘引导
```

`poweroff` 后 `vm` 服务会重启虚拟机;系统已在 `/dev/vda`,于是从磁盘而非 ISO 引导。SSH 由 `PORT_FWD`
转发(默认 `2222-22`,即宿主 `2222` → 客户机 `22`)。

---

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `OS` | `win11` | 设为 `alpine` 选中本客户机。 |
| `RAM_SIZE` | `2G` | 内存。 |
| `CPU_CORES` | `2` | 虚拟 CPU 数。 |
| `DISK_SIZE` | `8G` | 磁盘大小(仅首次)。 |
| `DISK` | `/storage/alpine/alpine.qcow2` | 磁盘路径。 |
| `NETWORK` | `user` | `user`(NAT + 端口转发)或 `none`。 |
| `PORT_FWD` | `2222-22` | user 模式宿主-客户机转发,逗号列表(如 `2222-22,8080-80`)。 |
| `VGA` | `std` | `std` 或 `virtio` 显示适配器。 |
| `CONSOLE` | `off` | `on` 暴露文本控制台(ttyS0)供 `vm-console`(见下)。 |
| `ALPINE_VERSION` | `3.21` | 无本地 ISO 时下载的版本。 |
| `ALPINE_ISO` | – | 显式 ISO 路径(跳过查找/下载)。 |
| `EXTRA_ARGS` | – | 额外原始 QEMU 参数。 |

网络、USB 和串口直通同样适用 [networking-and-devices.zh-CN.md](../common/networking-and-devices.zh-CN.md)
(与客户机无关)。Alpine 开箱只支持 `user`/`none`;要 `bridge`/`macvlan` 可用 `EXTRA_ARGS` 自行追加 netdev。

---

## 文本控制台(`vm-console`)

除了 noVNC 图形控制台,用 `CONSOLE=on` 启动可获得一个**串口文本控制台**,接终端即可 —— 无头 Linux VM 很方便:

```bash
docker exec -it alpine vm-console        # Ctrl-] 断开
```

Alpine 需在 `ttyS0` 上挂 getty 才会显示内容。最简单的办法:`setup-alpine` 时回答**串口**那一问(或之后在
`/etc/inittab` 加 `ttyS0`、在引导项加 `console=ttyS0`)。之后 `vm-console` 就能看到启动信息并通过终端登录。

---

## 新增客户机

Alpine 是非 Windows 客户机的模板:在 `rootfs/build/bin/` 放一个自行拼装 QEMU 命令的 `start-<os>` 脚本,
再在 `start-vm` 里加一个 `case`。引擎、控制台、网络与设备直通都与客户机无关。
