# vmd 引擎

[English](./engine.md) · **中文**

`vmd` 是一个静态 Rust 二进制,作为容器唯一服务运行(supervisord 程序 `vmd`,由 `ZSRV_vmd=true`
开关)。它负责:

- **QEMU 进程** —— 拉起、守护、按需重启;QMP 控制(含事件流);
- **sidecar** —— 内置 vTPM 的 swtpm,以及你自定义的进程;随 VM 退出后自动重生;
- `[web] port` 上的 **Web 控制台 + 电源 API** —— 内嵌 UI(noVNC + xterm),客户机关机时也保持在线。

vmd 不含任何特定系统的知识。一个客户机 = 一个 `[guest.<name>]` 块 + 一个 launcher 脚本。

## 配置

查找顺序:`$VMD_CONFIG` → `/vms/vmd.toml` → `/etc/vmd/vmd.toml`。首次 `run` 时,若挂载了 `/vms`
卷且其中没有配置,会自动把内置默认复制到 `/vms/vmd.toml` 供编辑。活动客户机:`$VMD_OS`,否则
`default`。

### 客户机字段

| 字段 | 含义 |
|---|---|
| `dir` | 客户机主目录。磁盘、状态、socket、日志、`scripts/` 都在其下。 |
| `disk` | 磁盘镜像。绝对路径原样用;相对路径在 `dir` 下;省略 → `{dir}/disk.qcow2`。 |
| `disk_size` | 全新安装时的大小(默认 16G)。以 `VMD_DISK_SIZE` 暴露。 |
| `ram`、`cpus` | 资源(默认 2G / 2)。 |
| `launch` | 启动器:模板名(`/build/templates/` 下的文件夹)或脚本路径。 |
| `qemu` | 内联 QEMU 命令(替代 `launch`)。必须含 `-qmp unix:{qmp},server,nowait`。 |
| `extra` | 追加到命令的额外参数(会做占位符替换)。 |
| `tpm` | `true` = 受管 swtpm sidecar;launcher 通过 `$VMD_TPM_SOCK` 接线。 |
| `seed` | `[{ template, to }]` 缺失才复制(如 OVMF NVRAM)。 |
| `prepare` | 启动前执行的一次性命令(如 `mkdir`)。 |
| `sidecars` | VM 旁的额外进程/脚本:`[{ command, wait_for }]`。 |

### 占位符 / 环境变量

每个占位符既可用于配置字符串替换,也以 `VMD_<KEY>` 导出给 launcher、安装脚本和 sidecar:

`accel cpu cpus ram name uuid mac state dir disk disk_size vnc_sock qmp console_sock tpm_sock
web_port`

`{state}` 是磁盘路径去扩展名(如 `/vms/win11/windows`),是所有状态文件的前缀。`accel`/`cpu`
自动探测(`kvm/host`,退回 `tcg/max`)。

## 脚本:模板 → 你的副本

`launch = "win11"` 指向模板文件夹 `/build/templates/win11/`(基路径可用 `$VMD_TEMPLATES` 覆盖)。
首次启动时 vmd 把文件夹内容复制到 **`{dir}/scripts/`** 并运行副本。各脚本槽位的解析顺序:

1. `{dir}/scripts/<槽位>` —— 你的副本;**永不覆盖**,编辑即生效;
2. 模板文件夹内的文件 —— 首启播种;
3. 自定义路径(`launch` / `install.launch` 为路径时)—— 同样播种进 `scripts/`。

launcher 必须 `exec qemu-system-x86_64 … -qmp "unix:${VMD_QMP},server,nowait"`。

## 安装把关

```toml
[guest.win11.install]
policy      = "auto"      # auto | force | none
# launch    = "win11"     # 安装脚本;默认取客户机自己的模板
source_iso  = "/images/win11.iso"
username    = "docker"
```

- **auto** —— 安装脚本只跑一次(标记 `{state}.installed`),之后跳过。标记仅在磁盘存在时生效:
  删掉磁盘,下次启动会自动重装。
- **force** —— 一次性抹盘重装(环境变量 `FORCE=1`),记录于 `{state}.force-applied`。
- **none** —— 永不安装;已有磁盘标记为 `migrated`,没有磁盘则报错。

表内其余每个键都**大写后作为环境变量**传给脚本(`source_iso` → `SOURCE_ISO`),值会做占位符
替换。磁盘与大小不用重复写——脚本读 `VMD_DISK` / `VMD_DISK_SIZE`。退出码 0 = 安装完成。

## 电源生命周期

- `POST /power/shutdown`(或 SIGTERM / `docker stop`):按下 ACPI 电源键 → 客户机关机。vmd 会
  **每 20 秒重按一次**(Windows 在登录界面初始化期间会丢弃该事件);若客户机**睡眠**则先唤醒,
  连续两次入睡则强制断电;超过 `stop_grace_secs`(默认 150)后 SIGKILL 兜底。
- 客户机干净关机**不会**结束容器:Web 控制台保持在线,`POST /power/start` 再次开机。
- `reset` = 硬复位,`poweroff` = 立即退出 QEMU。
- QEMU 非零退出 → vmd 以同码退出,supervisord 自动重启。

命令行(容器内):`vmd power status|start|shutdown|reset|poweroff`、`vmd print`。

## Web 端点

| 端点 | 作用 |
|---|---|
| `/` | 控制台 UI(首页 / VNC / 串口;中英文)。 |
| `/websockify` | WebSocket ↔ QEMU VNC unix socket(`{state}.vnc.sock`)。 |
| `/console` | WebSocket ↔ 串口 unix socket(`{state}.console.sock`)。 |
| `GET /status` | `running` / `off` / …… |
| `GET /info` | JSON:名称、资源、UUID/MAC、TPM、端口转发、串口可用性、完整命令。 |
| `POST /power/<a>` | `start` \| `shutdown` \| `reset` \| `poweroff`。 |

安全:VNC 与串口都是 **unix socket** —— 除 Web 端口外零 TCP 监听。所有端点执行同源/白名单
Origin 校验(`[web] allowed_origins` 追加)。`[web] password` 让一切先过登录(会话 cookie;
命令行走 `X-VMD-Password` 头)。密码连续输错会被递增延迟;经 HTTPS 反向代理
(`X-Forwarded-Proto: https`)时会话 cookie 自动带 `Secure`。vmd 本身只提供明文 HTTP ——
端口请保持在 localhost 或置于该代理之后。
