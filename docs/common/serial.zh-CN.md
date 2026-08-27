# 串口

[English](./serial.md) · **中文**

本文属于[设备指南](./devices.zh-CN.md)——客户机看到的一切设备都由它的 launcher
(`{dir}/scripts/launcher`)拼出;改完该文件后重启 VM 生效。

---

## 内置串口控制台

把 COM1 接到 vmd 的控制台 socket,Web 终端(`/console`)即可用:

```bash
-serial "unix:${VMD_CONSOLE_SOCK},server,nowait"
```

Alpine 模板默认如此(`console=ttyS0`)。Windows 用处有限,win11 模板未接——需要 COM1 就自行加上。

## 串口映射到 TCP

```bash
-serial tcp:0.0.0.0:4555,server,nowait      # 容器内的裸 TCP 服务
# telnet 形式:-serial telnet:0.0.0.0:4555,server,nowait
```

配合 `-p 127.0.0.1:4555:4555` 发布。任何连上该端口的连接都直通客户机 COM 口。

## 直通宿主机串口设备

先把设备映射进容器,再交给 QEMU:

```yaml
devices:
  - /dev/kvm
  - /dev/ttyUSB0            # 物理串口适配器
```

```bash
-serial /dev/ttyUSB0
```

## 多个 COM 口

每个 `-serial …` 依次是 COM1、COM2……;`-serial null` 跳过一个槽位。更多端口用
`-device pci-serial` 搭配显式 chardev。
