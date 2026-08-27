# 与宿主机共享文件(virtiofs)

[English](./file-sharing.md) · **中文**

本文属于[设备指南](./devices.zh-CN.md)——客户机看到的一切设备都由它的 launcher
(`{dir}/scripts/launcher`)拼出;改完该文件后重启 VM 生效。

---

`shares` 把宿主机目录直接导出给客户机——不用网络共享,也不用 SMB 服务:

```toml
[guest.alpine]
shares = [
  { tag = "host", path = "/shared" },              # cache = "auto"(默认) | "always" | "never"
  { tag = "iso",  path = "/images" },              # 可配多个,每个一个 tag
]
```

vmd 为每个 share 启动一个 **virtiofsd**(受管 sidecar:先于 QEMU 就绪,随 VM 一起重启),并把
`$VMD_FS_SOCKS` / `$VMD_FS_TAGS` 两个空格分隔的列表交给 launcher,由它拼成
`-chardev socket,… -device vhost-user-fs-pci,tag=<tag>`,外加一个共享的 `memory-backend-memfd`
(vhost-user-fs 只能映射*共享*后端里的客户机内存)。没有配 share 时两个列表为空,QEMU 命令保持原样。

把目录挂进容器即可,不需要额外 capability。vmd 用的是上游 Rust 版 **virtiofsd**,跑在 `chroot`
沙箱下,普通容器就允许(QEMU 自带的那个已废弃的 C 版则需要 `--cap-add SYS_ADMIN`):

```bash
docker run -d --name qemu --device=/dev/kvm \
  -e ZSRV_vmd=true -e VMD_OS=alpine \
  -v "$PWD/vms:/vms" -v "$PWD/shared:/shared" \
  -p 127.0.0.1:8006:8006 zzci/qemu
```

`path` 是**容器内**的路径,客户机看到什么由 bind mount 决定——只读共享用
`-v /my/data:/shared:ro`。

## 在客户机里挂载

Linux——tag 就是"设备名":

```bash
mount -t virtiofs host /mnt/host
echo "host /mnt/host virtiofs defaults 0 0" >> /etc/fstab    # 重启后仍然挂载
```

### Windows

Windows 要装两样东西才能用这个设备(PCI `1af4:105a`):**[WinFsp](https://winfsp.dev)**
(`virtiofs.exe` 依赖的 FUSE 层)和 **viofs** —— `virtio-win.iso` 里的 `VirtioFsDrv` 内核驱动
加 `virtiofs.exe` 用户态服务。

**用本镜像安装出来的 Windows 会自动装好这两样。** 安装脚本把 WinFsp(自动下载一次,可用
`[guest.win11.install]` 的 `winfsp_url` / `winfsp_sha256` 控制)和 viofs 文件塞进无人值守介质,
unattend 在首次登录时装好:驱动用 `pnputil`,服务用 `sc create VirtioFsSvc`。所以只要配上
`shares` 就行——宿主机目录会以 **`Z:`** 出现,卷标就是该 share 的 `tag`。

接线细节:

- 服务是**延迟自动启动**,且只依赖 `WinFsp.Launcher`,开机一分钟左右才起来。首次开机时 viofs
  驱动服务(`VirtioFsDrv`)还不存在——PnP 第一次看到设备时才创建它——普通 auto 启动会以 `1075`
  失败,必须重启一次才可用。另外还配了失败重试作为兜底。
- 它只挂**一个** share(`virtiofs.exe -m Z:`)。配多个 share 时,给每个额外的 tag 再建一个服务:
  `sc create VirtioFsSvc2 binPath= "\"C:\Program Files\Virtio-Win\VioFS\virtiofs.exe\" -t <tag> -m Y:" …`。
- 安装日志:`C:\Windows\Temp\virtiofs-setup.log`。状态:`sc.exe query VirtioFsSvc`(服务)、
  `sc.exe query VirtioFsDrv`(驱动,只有挂了 share 时才存在)。

#### 给已有的 Windows 补装

如果客户机是在这套流程之前装的,就手动来。运行期 launcher 没有光驱,先把驱动 ISO 加进 launcher
再重启 VM(也没有 SATA 控制器——自己加一个,或挂到模板已有的 `qemu-xhci` 上):

```bash
-device ahci,id=ahci \
-drive file=/images/virtio-win.iso,if=none,id=cd1,media=cdrom \
-device ide-cd,drive=cd1,bus=ahci.0
```

然后在客户机的**管理员**命令提示符里(驱动 ISO 在 `E:`):

```bat
# 1. 内核驱动 —— 或者直接运行 E:\virtio-win-guest-tools.exe,它也会装这个驱动
pnputil /add-driver E:\viofs\w11\amd64\viofs.inf /install

# 2. 用户态服务 —— guest-tools 的 MSI 里没有 virtiofs.exe,要从 ISO 里复制出来
mkdir "C:\Program Files\Virtio-Win\VioFS"
copy E:\viofs\w11\amd64\virtiofs.exe "C:\Program Files\Virtio-Win\VioFS\"
sc.exe create VirtioFsSvc binPath="C:\Program Files\Virtio-Win\VioFS\virtiofs.exe" ^
  start=auto depend="WinFsp.Launcher/VirtioFsDrv" DisplayName="Virtio FS Service"
sc.exe start VirtioFsSvc
```

WinFsp 也要装(驱动 ISO 里没有)。只是偶尔传个文件的话,RDP 驱动器重定向(`mstsc` → 本地资源 →
驱动器)在客户机里什么都不用装。

## 取舍

- **挂了 share 期间 `power save` 不可用。** `vhost-user-fs` 不支持迁移,QEMU 会以
  `State blocked by non-migratable device 'vhost-user-fs'` 拒绝保存状态,vmd 日志显示
  `save failed … VM keeps running`。需要保存/恢复就去掉 `shares`。
- 客户机内存改由 `memory-backend-memfd` 提供(大小不变,标记为共享)——ACPI 关机/重启不受影响。
- 每个 share 一个 virtiofsd 进程,`vmd print` 里能看到完整命令。vmd 固定用
  `--sandbox chroot --modcaps=-DAC_READ_SEARCH --inode-file-handles=never` 启动——这是默认
  Docker capability 集允许的组合。代价是放弃 file handle 优化(改为每个 inode 占一个打开的 fd),
  只有目录树非常大时才有影响。
