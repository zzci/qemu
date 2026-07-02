<!--
  客户机文档模板。复制为 docs/guests/<os>.zh-CN.md(及 <os>.md),填好占位符,再从 README 链接。
  保留第 3 行的语言切换。删除本注释。
-->
# <名称> 客户机(`VMD_OS=<os>`)

[English](./<os>.md) · **中文**

一句话:这是什么客户机、如何安装(无人值守 / 云镜像 / 现成磁盘)。

引擎总览:[根 README](../../README.zh-CN.md);系统无关部分(配置、脚本、生命周期、Web 控制台):
[common/engine.zh-CN.md](../common/engine.zh-CN.md)。本页只写 `<os>` 特有内容。

## 配置

```toml
[guest.<os>]
dir       = "/vms/<os>"
disk      = "<os>.qcow2"
disk_size = "…"
ram       = "…"
cpus      = 2
launch    = "<os>"

[guest.<os>.install]
policy = "auto"
# 客户机特有键 —— 每个都会大写后作为环境变量传给 {dir}/scripts/install
```

| install 键 | 环境变量 | 默认 | 说明 |
|---|---|---|---|
| … | … | … | … |

## 安装脚本做了什么

介质处理(下载 / `/images` 查找 / 重打包),以及"安装完成"的定义(退出码 0)。

## 访问

Web 控制台 / 串口 / SSH / RDP —— 按实际情况写,并注明 launcher 转发的端口。

网络与设备直通是系统无关的 —— 见
[common/networking-and-devices.zh-CN.md](../common/networking-and-devices.zh-CN.md)。

## 排障

| 症状 | 原因 / 处理 |
|------|-------------|
| … | … |
