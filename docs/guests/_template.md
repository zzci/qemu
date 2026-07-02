<!--
  Guest doc template. Copy to docs/guests/<os>.md (+ <os>.zh-CN.md), fill the placeholders, then
  list it in docs/README.md. Keep the language toggle on line 3. Delete this comment.
-->
# <Name> guest (`OS=<os>`)

**English** · [中文](./<os>.zh-CN.md)

One sentence: what this guest is and how it installs (unattended / console / pre-built image).

See the [root README](../../README.md) for the engine overview and
[common/engine.md](../common/engine.md) for the OS-agnostic parts (dispatch, console, config/state
convention, storage layout). This page is `<os>`-specific.

---

## How it works

What `start-<os>` does: firmware (SeaBIOS / OVMF+TPM), media handling (download / `/images` lookup /
repack), device model, and how the disk vs. install media boot order is arranged.

---

## Install

```bash
docker run -d --name <os> --device=/dev/kvm -e OS=<os> \
  -v "$PWD/storage-<os>:/storage" -p 8006:8006 zzci/qemu
```

Steps the user takes (unattended → nothing; console → the commands to run in noVNC), and how it
reaches a booted state.

---

## Environment variables

Only the knobs `start-<os>` actually reads. Engine-wide knobs (RAM_SIZE, CPU_CORES, NETWORK,
DISK, EXTRA_ARGS, …) are in the [configuration table](../../README.md#configuration); list the
guest-specific ones here.

| Variable | Default | Description |
|----------|---------|-------------|
| `OS` | `win11` | Set to `<os>` to select this guest. |
| `DISK` | `/storage/<os>/<os>.qcow2` | Disk path. |
| … | … | … |

Networking and device passthrough are guest-agnostic — see
[common/networking-and-devices.md](../common/networking-and-devices.md).

---

## Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| … | … |
