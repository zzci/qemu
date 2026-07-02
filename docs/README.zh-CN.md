# 文档

[English](./README.md) · **中文**

`zzci/qemu` 引擎的深入指南,分为**系统无关**(引擎本身,所有客户机共用)与**每客户机**(每个 `OS` 一篇)。
请先看[根 README](../README.zh-CN.md) 了解概览。每份指南都有英文版与中文版(`*.zh-CN.md`),顶部互相链接。

## 通用(系统无关)

| 指南 | 内容 |
|------|------|
| [common/engine.zh-CN.md](./common/engine.zh-CN.md) | 引擎本身 —— 分派(`start-vm`)、服务与控制台、KVM、每机配置/状态约定、存储布局,以及**如何新增客户机**。 |
| [common/networking-and-devices.zh-CN.md](./common/networking-and-devices.zh-CN.md) | 网络模式、USB 直通、串口,以及 `EXTRA_ARGS` 万能逃生口 —— 附对应的 `docker` 设备/权限参数。 |

## 客户机(每系统)

| 指南 | 内容 |
|------|------|
| [guests/windows.zh-CN.md](./guests/windows.zh-CN.md) | Windows 11 LTSC —— 安装策略与安装状态、ISO、语言/版本、显示(`std`→`virtio`)、账号/RDP、存储与每机配置、克隆、排错。 |
| [guests/alpine.zh-CN.md](./guests/alpine.zh-CN.md) | Alpine Linux —— 控制台安装、环境变量,以及新增客户机的最简模板。 |

**要新增一个系统?** 读 [common/engine.zh-CN.md → 新增客户机](./common/engine.zh-CN.md#新增客户机):
写一个 `start-<os>` 脚本,在 `start-vm` 里加一个分支,再加 `docs/guests/<os>.md`。上面的通用文档已经覆盖
所有与客户机无关的内容。
