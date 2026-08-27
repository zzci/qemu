# USB

[English](./usb.md) · **中文**

本文属于[设备指南](./devices.zh-CN.md)——客户机看到的一切设备都由它的 launcher
(`{dir}/scripts/launcher`)拼出;改完该文件后重启 VM 生效。

---

win11 模板已带 USB 控制器和平板指针:

```bash
-device qemu-xhci -device usb-tablet
```

## 宿主机 USB 直通

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

> **顺序很重要** —— `usb-host`(以及任何需要 USB 总线的设备)必须排在 `-device qemu-xhci` **之后**。
> QEMU 按顺序实例化 `-device`,`usb-host` 排在控制器**前面**会报 `No 'usb-bus' bus found for device
> 'usb-host'`。要加到运行期 **launcher**(它有 `qemu-xhci`),**不要**加到精简的装机阶段(没有 USB
> 控制器)。只有一个控制器时,直接 `vendorid/productid` 会自动挂上 —— 显式 `bus=xhci.0`(加上控制器
> `id=xhci`)只在**多个控制器**需要消歧义时才用。

USB3 设备经 qemu-xhci 直接可用。等时传输设备(音频、摄像头)效果不一,优先按总线/端口挂载。

## 镜像文件模拟 U 盘

```bash
-drive file=/vms/win11/usbdisk.img,if=none,id=usb1,format=raw \
-device usb-storage,drive=usb1
```
