# 设备

[English](./devices.md) · **中文**

客户机看到的一切设备都由**它的 launcher 脚本**决定 —— `{dir}/scripts/launcher`,首启从模板播种
的可编辑副本。改任何设备:编辑该文件,然后重启 VM(`shutdown` + `start`,或重启容器)。
`vmd print` 可在不运行的情况下查看解析后的完整命令。

launcher 通过 `VMD_*` 环境变量拿到参数(`VMD_MAC`、`VMD_QMP`、`VMD_CONSOLE_SOCK` 等)——见
[engine.zh-CN.md](./engine.zh-CN.md)。

> **不要在 launcher 里用 `#` 注释某一行设备。** `qemu-system-x86_64 … \` 是用 `\` 续行拼成的**一条**
> 逻辑命令;shell 拼接后,一个 `#` 会把**它之后的全部内容**(包括 `-qmp`、磁盘等)都注释掉,VM 直接坏。
> 要开关某个设备:**删掉那一行**,或像 launcher 里 `INCOMING=()` 那样用 env 驱动的 bash 数组来控制。

## 分类指南

| 指南 | 内容 |
|------|------|
| [networking.zh-CN.md](./networking.zh-CN.md) | 用户态/NAT 与端口转发、桥接/tap 与 macvlan、多网卡。 |
| [serial.zh-CN.md](./serial.zh-CN.md) | 内置 Web 串口控制台、COM → TCP、宿主串口直通、多 COM 口。 |
| [usb.zh-CN.md](./usb.zh-CN.md) | xhci 控制器与平板指针、按 id 或总线/端口直通、镜像文件模拟 U 盘。 |
| [file-sharing.zh-CN.md](./file-sharing.zh-CN.md) | virtiofs `shares` —— 把宿主机目录直接挂进客户机。 |

其余设备篇幅很小,直接写在本文。

## 显示

模板使用 `-device "virtio-vga,xres=1920,yres=1080"`,改数字即改默认分辨率(virtio GPU 驱动装好后
客户机内也能切换)。画面输出到 VNC unix socket,由 Web 控制台桥接;宿主机上没有显示窗口。

GPU 直通(VFIO)目前引擎还没有接入——它需要宿主机 IOMMU 配置,并给容器映射 `--device /dev/vfio/…`,
到时会单独成篇。

## 额外磁盘

加一对 `-drive file=…,if=none,id=disk1 -device virtio-blk-pci,drive=disk1`;镜像在 `prepare` 里
创建(`qemu-img create -f qcow2 {dir}/data.qcow2 100G`)。

## 光驱

运行期 launcher 没有 SATA 控制器(只有精简的装机阶段有),所以要自己加一个:

```bash
-device ahci,id=ahci \
-drive file=/images/tools.iso,if=none,id=cd1,media=cdrom \
-device ide-cd,drive=cd1,bus=ahci.0
```

或者挂到 win11 模板已有的 USB 控制器上:`-device usb-storage,drive=cd1`。

## 声音

```bash
-audiodev none,id=snd0 -device ich9-intel-hda -device hda-output,audiodev=snd0
```

VNC 不传声音;Windows 建议用 RDP 听声。

---

改完先 `vmd print` 检查命令,再重启客户机生效。
