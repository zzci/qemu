# Documentation

**English** · [中文](./README.zh-CN.md)

In-depth guides for the `zzci/qemu` engine, split into **OS-agnostic** (the engine, shared by every
guest) and **per-guest** (one document per `OS`). Start with the [root README](../README.md) for the
overview. Each guide has an English version and a Chinese version (`*.zh-CN.md`), cross-linked at the
top.

## Common (OS-agnostic)

| Guide | What it covers |
|-------|----------------|
| [common/engine.md](./common/engine.md) | The engine itself — dispatch (`start-vm`), services & console, KVM, the per-VM config/state convention, storage layout, and **how to add a new guest**. |
| [common/networking-and-devices.md](./common/networking-and-devices.md) | Networking modes, USB passthrough, serial ports, and the `EXTRA_ARGS` escape hatch — with the matching `docker` device/cap flags. |

## Guests (per-OS)

| Guide | What it covers |
|-------|----------------|
| [guests/windows.md](./guests/windows.md) | Windows 11 LTSC — install policy & install-state, ISO, locale/edition, display (`std`→`virtio`), accounts/RDP, storage & per-VM config, clones, troubleshooting. |
| [guests/alpine.md](./guests/alpine.md) | Alpine Linux — console install, env vars, and the minimal template for adding a guest. |

**Adding a new OS?** Read [common/engine.md → Adding a guest](./common/engine.md#adding-a-guest):
write a `start-<os>` script, wire a case into `start-vm`, then add `docs/guests/<os>.md`. The common
docs above already cover everything that is guest-agnostic.
