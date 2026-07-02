<!--
  Guest doc template. Copy to docs/guests/<os>.md (+ <os>.zh-CN.md), fill the placeholders, then
  link it from the READMEs. Keep the language toggle on line 3. Delete this comment.
-->
# <Name> guest (`VMD_OS=<os>`)

**English** · [中文](./<os>.zh-CN.md)

One sentence: what this guest is and how it installs (unattended / cloud image / pre-built disk).

Engine overview: [root README](../../README.md); OS-agnostic parts (config, scripts, lifecycle,
web console): [common/engine.md](../common/engine.md). This page is `<os>`-specific.

## Configuration

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
# guest-specific keys — each becomes an UPPERCASE env var for {dir}/scripts/install
```

| Install key | Env var | Default | Description |
|---|---|---|---|
| … | … | … | … |

## What the installer does

Media handling (download / `/images` lookup / repack), and what "installed" means (exit 0).

## Access

Web console / serial / SSH / RDP — whatever applies, with the ports the launcher forwards.

Networking and device passthrough are guest-agnostic — see
[common/networking-and-devices.md](../common/networking-and-devices.md).

## Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| … | … |
