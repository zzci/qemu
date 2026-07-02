# 文档

[English](./README.md) · **中文**

`zzci/qemu` 引擎的深入指南,分为**系统无关**(vmd 引擎,所有客户机共用)与**按客户机**
(每个 `VMD_OS` 一篇)。总览请先看[根 README](../README.zh-CN.md)。每篇都有英文版与中文版
(`*.zh-CN.md`),顶部互链。

## 通用(系统无关)

| 指南 | 内容 |
|------|------|
| [common/engine.zh-CN.md](./common/engine.zh-CN.md) | vmd 引擎 —— 配置(`vmd.toml`、占位符、`VMD_*` 环境变量)、模板脚本与 `{dir}/scripts`、安装把关、电源生命周期、Web 控制台与 API、安全。 |
| [common/networking-and-devices.zh-CN.md](./common/networking-and-devices.zh-CN.md) | 网络模式(用户态/NAT、桥接/tap、macvlan、多网卡)、串口映射(控制台 / TCP / 宿主设备)、USB 直通、额外磁盘、显示、声音。 |

## 客户机(按系统)

| 指南 | 内容 |
|------|------|
| [guests/windows.zh-CN.md](./guests/windows.zh-CN.md) | Windows 11 —— 无人值守安装(`[guest.win11.install]` 键)、安装脚本流程、RDP、克隆、排障。 |
| [guests/alpine.zh-CN.md](./guests/alpine.zh-CN.md) | Alpine Linux —— 零介质云镜像安装、串口控制台、最简模板。 |

**新增系统?**见 [CONTRIBUTING.md](./CONTRIBUTING.md):建一个模板文件夹(`launcher` + 可选
`install`),加一个 `[guest.<name>]` 块即可 —— 无需改引擎。
