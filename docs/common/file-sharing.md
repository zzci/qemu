# Sharing files with the host (virtiofs)

**English** · [中文](./file-sharing.zh-CN.md)

Part of the [device guides](./devices.md) — everything the guest sees is built by its launcher
(`{dir}/scripts/launcher`); edit that file and power-cycle the VM to apply.

---

`shares` exports host directories straight into the guest — no network share, no SMB server:

```toml
[guest.alpine]
shares = [
  { tag = "host", path = "/shared" },              # cache = "auto" (default) | "always" | "never"
  { tag = "iso",  path = "/images" },              # any number of shares, one tag each
]
```

vmd starts one **virtiofsd** per share (a managed sidecar: ready before QEMU, respawned with the
VM) and hands the launcher `$VMD_FS_SOCKS` / `$VMD_FS_TAGS` — two space-separated lists it turns
into `-chardev socket,… -device vhost-user-fs-pci,tag=<tag>` pairs, plus a shared
`memory-backend-memfd` (vhost-user-fs can only map guest RAM out of a *shared* backend). With no
share configured both lists are empty and the QEMU command is unchanged.

Just mount the directory into the container — no extra capability is needed. vmd runs upstream's
Rust **virtiofsd** in its `chroot` sandbox, which a stock container already allows (QEMU's own,
deprecated C daemon would have needed `--cap-add SYS_ADMIN`):

```bash
docker run -d --name qemu --device=/dev/kvm \
  -e ZSRV_vmd=true -e VMD_OS=alpine \
  -v "$PWD/vms:/vms" -v "$PWD/shared:/shared" \
  -p 127.0.0.1:8006:8006 zzci/qemu
```

`path` is a path **inside the container**, so the bind mount decides what the guest sees — use
`-v /my/data:/shared:ro` for a read-only share.

## Mounting it in the guest

Linux — the tag is the "device":

```bash
mount -t virtiofs host /mnt/host
echo "host /mnt/host virtiofs defaults 0 0" >> /etc/fstab    # persist across reboots
```

### Windows

Windows needs two pieces before it can touch the device (PCI `1af4:105a`): **[WinFsp](https://winfsp.dev)**
(the FUSE layer `virtiofs.exe` links against) and **viofs** — the `VirtioFsDrv` kernel driver plus
the `virtiofs.exe` user-space service from `virtio-win.iso`.

**A Windows guest installed by this image gets both automatically.** The installer stages WinFsp
(downloaded once; `winfsp_url` / `winfsp_sha256` in `[guest.win11.install]`) and the viofs bits into
the unattended media, and the unattend installs them at first logon: driver via `pnputil`, service
via `sc create VirtioFsSvc`. So configuring `shares` is all it takes — the host directory appears as
**`Z:`**, with the share's `tag` as the volume label.

Notes on how it is wired:

- The service is **delayed-auto** and only depends on `WinFsp.Launcher`, so it starts a minute or
  so after boot. On the very first boot the viofs driver service (`VirtioFsDrv`) does not exist yet
  — PnP creates it when it first sees the device — and a plain auto-start service would die with
  `1075` and only work after a reboot. Failure actions retry the start as a second safety net.
- It mounts **one** share (`virtiofs.exe -m Z:`). With several `shares`, add a service per extra
  tag: `sc create VirtioFsSvc2 binPath= "\"C:\Program Files\Virtio-Win\VioFS\virtiofs.exe\" -t <tag> -m Y:" …`.
- Setup log: `C:\Windows\Temp\virtiofs-setup.log`. State: `sc.exe query VirtioFsSvc` (service) and
  `sc.exe query VirtioFsDrv` (driver, exists only while a share is attached).

#### Retrofitting an existing Windows guest

For a guest installed before this was in place, do it by hand. The runtime launcher has no CD-ROM,
so add the driver ISO to it and power-cycle the VM (there is no SATA controller either — add one,
or hang the drive off the template's `qemu-xhci`):

```bash
-device ahci,id=ahci \
-drive file=/images/virtio-win.iso,if=none,id=cd1,media=cdrom \
-device ide-cd,drive=cd1,bus=ahci.0
```

Then, in an **administrator** command prompt in the guest (driver ISO at `E:`):

```bat
# 1. kernel driver — or just run E:\virtio-win-guest-tools.exe, which installs it as well
pnputil /add-driver E:\viofs\w11\amd64\viofs.inf /install

# 2. user-space service — virtiofs.exe is NOT in the guest-tools MSI, copy it off the ISO
mkdir "C:\Program Files\Virtio-Win\VioFS"
copy E:\viofs\w11\amd64\virtiofs.exe "C:\Program Files\Virtio-Win\VioFS\"
sc.exe create VirtioFsSvc binPath="C:\Program Files\Virtio-Win\VioFS\virtiofs.exe" ^
  start=auto depend="WinFsp.Launcher/VirtioFsDrv" DisplayName="Virtio FS Service"
sc.exe start VirtioFsSvc
```

WinFsp has to be installed too (it is not on the driver ISO). For a one-off file transfer, RDP
drive redirection (`mstsc` → Local Resources → Drives) needs nothing in the guest at all.

## Trade-offs

- **`power save` is unavailable while a share is attached.** `vhost-user-fs` is not migratable, so
  QEMU rejects the state save with `State blocked by non-migratable device 'vhost-user-fs'` and
  vmd logs `save failed … VM keeps running`. Remove the `shares` entry if you need save/resume.
- Guest RAM moves into a `memory-backend-memfd` object (same size, shared) — plain ACPI
  shutdown/reset are unaffected.
- One virtiofsd process per share, all visible in `vmd print`. vmd runs each with
  `--sandbox chroot --modcaps=-DAC_READ_SEARCH --inode-file-handles=never` — the combination a
  default Docker capability set allows. The cost is the file-handle optimization (virtiofsd keeps
  an open fd per inode instead), which only shows up on very large trees.
