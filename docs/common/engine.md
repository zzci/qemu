# The vmd engine

**English** · [中文](./engine.zh-CN.md)

`vmd` is a single static Rust binary running as the container's only service (supervisord program
`vmd`, toggled by `ZSRV_vmd=true`). It owns:

- the **QEMU process** — spawn, supervise, restart-on-start; QMP control (events included);
- **sidecars** — swtpm for the built-in vTPM, plus anything you list; respawned automatically if
  they die with the VM;
- the **web console + power API** on `[web] port` — embedded UI (noVNC + xterm), stays up even
  while the guest is off.

vmd has no per-OS knowledge. A guest = a `[guest.<name>]` block + a launcher script.

## Configuration

Search order: `$VMD_CONFIG` → `/vms/vmd.toml` → `/etc/vmd/vmd.toml`. On first `run`, the baked
default is copied to `/vms/vmd.toml` (if a `/vms` volume exists and nothing is there) so you edit a
live copy. Active guest: `$VMD_OS`, else `default`.

### Guest fields

| Field | Meaning |
|---|---|
| `dir` | Guest home. Disk, state, sockets, logs and `scripts/` live under it. |
| `disk` | Disk image. Absolute as-is; relative under `dir`; omitted → `{dir}/disk.qcow2`. |
| `disk_size` | Size for fresh installs (default 16G). Exposed as `VMD_DISK_SIZE`. |
| `ram`, `cpus` | Resources (default 2G / 2). |
| `launch` | Launcher: a bare template name (folder under `/build/templates/`) or a script path. |
| `qemu` | Inline QEMU command instead of `launch`. Must include `-qmp unix:{qmp},server,nowait`. |
| `extra` | Extra args appended to the command (placeholder-substituted). |
| `tpm` | `true` = managed swtpm sidecar; the launcher wires it via `$VMD_TPM_SOCK`. |
| `seed` | `[{ template, to }]` copy-if-missing (e.g. OVMF NVRAM). |
| `prepare` | Commands run once before boot (e.g. `mkdir`). |
| `sidecars` | Extra processes/scripts beside the VM: `[{ command, wait_for }]`. |

### Placeholders / env

Every placeholder is substituted in config strings **and** exported to the launcher, install
script and sidecars as `VMD_<KEY>`:

`accel cpu cpus ram name uuid mac state dir disk disk_size vnc_sock qmp console_sock tpm_sock
web_port`

`{state}` is the disk path without extension (e.g. `/vms/win11/windows`) — the stem for all state
files. `accel`/`cpu` are auto-detected (`kvm/host`, falling back to `tcg/max`).

## Scripts: template → your copy

`launch = "win11"` points at the template folder `/build/templates/win11/` (base overridable with
`$VMD_TEMPLATES`). On first boot vmd copies the folder's files into **`{dir}/scripts/`** and runs
the copies. Resolution per script slot:

1. `{dir}/scripts/<slot>` — your copy; **never overwritten**, edits win;
2. the template folder file — seeded on first boot;
3. a custom path (when `launch` / `install.launch` is a path) — seeded into `scripts/` too.

The launcher must `exec qemu-system-x86_64 … -qmp "unix:${VMD_QMP},server,nowait"`.

## Install gating

```toml
[guest.win11.install]
policy      = "auto"      # auto | force | none
# launch    = "win11"     # install script; defaults to the guest's own template
source_iso  = "/images/win11.iso"
username    = "docker"
```

- **auto** — run the install script once (marker `{state}.installed`), then skip. The marker only
  counts while the disk exists: delete the disk and the next boot reinstalls automatically.
- **force** — one-shot wipe + reinstall (`FORCE=1` in the env), recorded in
  `{state}.force-applied`.
- **none** — never install; an existing disk is marked `migrated`, a missing disk is an error.

Every other key in the table becomes an **UPPERCASE env var** for the script (`source_iso` →
`SOURCE_ISO`), values placeholder-substituted. Disk and size are not repeated — the script reads
`VMD_DISK` / `VMD_DISK_SIZE`. Exit 0 = installed.

## Power lifecycle

- `POST /power/shutdown` (or SIGTERM / `docker stop`): ACPI power button → the guest shuts down.
  vmd **re-presses the button every 20 s** (Windows drops the event while the logon UI is
  starting), wakes a **sleeping** guest first and force-quits if it suspends twice, and SIGKILLs
  after `stop_grace_secs` (default 150) as the last resort.
- A clean guest power-off does **not** end the container: the web console stays up and
  `POST /power/start` boots the VM again.
- `reset` = hard reset, `poweroff` = immediate QEMU quit.
- QEMU exit ≠ 0 → vmd exits with that code and supervisord restarts it.

CLI (inside the container): `vmd power status|start|shutdown|reset|poweroff`, `vmd print`.

## Web endpoints

| Endpoint | Purpose |
|---|---|
| `/` | Console UI (home / VNC / serial; English/中文). |
| `/websockify` | WebSocket ↔ QEMU VNC unix socket (`{state}.vnc.sock`). |
| `/console` | WebSocket ↔ serial console unix socket (`{state}.console.sock`). |
| `GET /status` | `running` / `off` / … |
| `GET /info` | JSON: name, resources, UUID/MAC, TPM, port forwards, serial-console availability, full command. |
| `POST /power/<a>` | `start` \| `shutdown` \| `reset` \| `poweroff`. |

Security: VNC and serial are **unix sockets** — nothing listens on TCP except the web port. All
endpoints enforce a same-origin/allowlist Origin check (`[web] allowed_origins` adds extras).
`[web] password` gates everything behind a login (session cookie; the CLI sends
`X-VMD-Password`). Wrong passwords are tarpitted with a growing delay; behind an HTTPS reverse
proxy (`X-Forwarded-Proto: https`) the session cookie is set `Secure`. vmd itself serves plain
HTTP — keep the port on localhost or behind that proxy.
