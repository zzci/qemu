<!--
  客户机文档模板。复制为 docs/guests/<os>.zh-CN.md(及 <os>.md),填好占位符,再到
  docs/README.zh-CN.md 里登记。保留第 3 行的语言切换。删除本注释。
-->
# <名称> 客户机(`OS=<os>`)

[English](./<os>.md) · **中文**

一句话:这是什么客户机、如何安装(无人值守 / 控制台 / 预装镜像)。

引擎概览见[根 README](../../README.zh-CN.md),系统无关部分(分派、控制台、配置/状态约定、存储布局)见
[common/engine.zh-CN.md](../common/engine.zh-CN.md)。本页是 `<os>` 专属内容。

---

## 工作原理

`start-<os>` 做什么:固件(SeaBIOS / OVMF+TPM)、介质处理(下载 / `/images` 查找 / 重制)、设备模型,
以及磁盘与安装介质的引导顺序如何安排。

---

## 安装

```bash
docker run -d --name <os> --device=/dev/kvm -e OS=<os> \
  -v "$PWD/storage-<os>:/storage" -p 8006:8006 zzci/qemu
```

用户要做的步骤(无人值守 → 无需操作;控制台 → 在 noVNC 里运行的命令),以及如何到达已引导状态。

---

## 环境变量

只列 `start-<os>` 实际读取的旋钮。引擎通用旋钮(RAM_SIZE、CPU_CORES、NETWORK、DISK、EXTRA_ARGS……)
在[配置表](../../README.zh-CN.md#配置)里;此处列本客户机专属的。

| 变量 | 默认 | 说明 |
|------|------|------|
| `OS` | `win11` | 设为 `<os>` 选中本客户机。 |
| `DISK` | `/storage/<os>/<os>.qcow2` | 磁盘路径。 |
| … | … | … |

网络与设备直通与客户机无关 —— 见
[common/networking-and-devices.zh-CN.md](../common/networking-and-devices.zh-CN.md)。

---

## 排错

| 现象 | 原因 / 处理 |
|------|-------------|
| … | … |
