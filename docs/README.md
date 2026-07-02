# Documentation

**English** · [中文](./README.zh-CN.md)

In-depth guides for the `zzci/qemu` engine, split into **OS-agnostic** (the vmd engine, shared by
every guest) and **per-guest** (one document per `VMD_OS`). Start with the
[root README](../README.md) for the overview. Each guide has an English version and a Chinese
version (`*.zh-CN.md`), cross-linked at the top.

## Common (OS-agnostic)

| Guide | What it covers |
|-------|----------------|
| [common/engine.md](./common/engine.md) | The vmd engine — configuration (`vmd.toml`, placeholders, `VMD_*` env), template scripts & `{dir}/scripts`, install gating, power lifecycle, web console & API, security. |
| [common/networking-and-devices.md](./common/networking-and-devices.md) | Network modes (user/NAT, bridge/tap, macvlan, multi-NIC), serial port mapping (console / TCP / host device), USB passthrough, extra disks, display, audio. |

## Guests (per-OS)

| Guide | What it covers |
|-------|----------------|
| [guests/windows.md](./guests/windows.md) | Windows 11 — unattended install (`[guest.win11.install]` keys), what the installer does, RDP, cloning, troubleshooting. |
| [guests/alpine.md](./guests/alpine.md) | Alpine Linux — zero-media cloud-image install, serial console, the minimal template. |

**Adding a new OS?** See [CONTRIBUTING.md](./CONTRIBUTING.md): create a template folder
(`launcher` + optional `install`), add a `[guest.<name>]` block, done — no engine changes.
