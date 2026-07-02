# vmd — Rust QEMU VM supervisor

A single Rust binary that replaces the bash scripts, `websockify` and `socat`. It owns the QEMU
process (and `swtpm`), drives the power lifecycle, and serves the consoles + a power API on one web
port. Configuration is **file-driven** (`vmd.toml`); no per-knob environment variables.

## Integration — one service

It runs as the **single supervisord program `vmd`**, toggled by ubase `ZSRV_vmd`. This replaces the
legacy `vm` / `tpm` / `novnc` services (vmd manages QEMU, swtpm and the web bridge itself):

```ini
# rootfs/build/services/vmd.conf
[program:vmd]
command=/build/bin/vmd run
autorestart=unexpected   # exit 0 = clean poweroff (stay off); non-zero = crash (restart)
exitcodes=0
```

`vmd` propagates QEMU's exit code, so the same stay-off-vs-restart semantics carry over. On
`docker stop`, supervisord sends SIGTERM → vmd asks the guest to ACPI power off, waits the grace
period, then SIGKILLs.

## Config (`vmd.toml`) — vmd has **no per-OS logic**

Each guest declares its QEMU command + setup as **data**; vmd fills a fixed set of `{placeholders}`
and runs it. Adding a guest is a config edit, never a Rust change.

- `qemu` — the QEMU command line (device model as data). Must include `-qmp unix:{qmp},server,nowait`.
- `extra` — extra QEMU args appended to `qemu` (add a device without editing the main command).
- `install` — `{ policy, command }`: vmd runs the command (with `FORCE` in env) if the disk isn't
  ready; the command carries all OS-specific install bits.
- `seed` — copy a template → per-VM path if missing (e.g. OVMF NVRAM).
- `prepare` — one-shot commands before QEMU (e.g. `mkdir`, tap/bridge setup).
- `sidecars` — processes started before QEMU and killed after (e.g. swtpm); optional `wait_for` path.

Placeholders vmd provides: `{accel} {cpu} {cpus} {ram} {name} {uuid} {mac} {state} {disk}
{disk_size} {vnc_host} {qmp} {console_sock} {web_port}`. `default` (or `-e VMD_OS=<name>`) selects the
active guest; search order `$VMD_CONFIG` → `/storage/vmd.toml` → `/etc/vmd/vmd.toml`.

**Seeing the exact command:** `vmd print` resolves and prints the whole plan + the final QEMU command
(dry run, nothing launched); every `vmd run` also writes the launched command to `{state}.qemu.cmd`.

## Layout

```
supervisor/
├── Cargo.toml          # tokio + serde + toml + anyhow; release = static musl, stripped
├── vmd.toml            # guests as data (qemu/install/seed/prepare/sidecars)
└── src/
    ├── main.rs         # dispatch + the generic run flow (install→seed→prepare→sidecars→qemu→supervise)
    ├── config.rs       # vmd.toml loader (OS-agnostic)
    ├── subst.rs        # {placeholder} substitution + tokenize
    ├── install.rs      # install gate (policy) -> external command -> result check
    ├── seed.rs         # copy-if-missing (per-VM file seeding)
    ├── sidecar.rs      # start/stop sidecar processes, optional wait_for
    ├── lifecycle.rs    # QEMU supervision + power lifecycle (signals, grace, exit codes)
    ├── qmp.rs          # minimal QMP client (powerdown / reset / quit / status)
    ├── util.rs         # persistent UUID, stable MAC
    └── log.rs          # [vmd] logging
```

## Lifecycle → QMP

vmd runs a **boot → supervise → idle** loop: a clean guest power-off does *not* stop vmd — the web
console stays up (status `off`) and `POST /power/start` re-spawns QEMU. `docker stop` (SIGTERM) exits
0; a QEMU crash exits non-zero so supervisord restarts vmd.

| Action       | Implementation                              |
|--------------|---------------------------------------------|
| start        | (re)spawn QEMU with `-qmp`, then `supervise()` |
| shutdown     | `system_powerdown` (ACPI) + grace → SIGKILL |
| hard reset   | `system_reset`                              |
| force off    | `quit` / SIGKILL                            |
| status       | `query-status` (or `off` while idle)        |

## Status / roadmap

- [x] **Phase 1** — crate, config (file-driven), QMP client, process lifecycle, `run` (minimal boot),
  `power` CLI, the `vmd` service definition.
- [x] **Phase 2** — **generic, config-driven** device model: vmd substitutes `{placeholders}` into
  the guest's `qemu` command and runs its `seed` / `prepare` / `sidecars` — **no OS logic in Rust**
  (the OVMF/TPM/device model live in `vmd.toml` data; swtpm is just a sidecar with `wait_for`).
  Verified end to end: alpine (SeaBIOS) and win11 (OVMF seed + real swtpm sidecar + tpm-tis chain),
  stable MAC + persistent UUID, sidecar reaped with no leak; `clippy -- -D warnings` clean;
  `#![forbid(unsafe_code)]`. Adding/fixing a guest = config only (proven: the swtpm dir fix was a
  `prepare` line, not a code change).
- [x] **Phase 3** — web server on `web.port` (axum): noVNC static + WS↔VNC bridge (replaces
  websockify), serial-console WS (replaces socat), power API (`POST /power/{shutdown,reset,poweroff}`,
  `GET /status`). Verified end to end against a fake QMP+VNC: `/status`→running, `/power/*`→QMP.
- [x] **Phase 4** — install orchestration (gate on policy → external command → result → marker) +
  `vmd print`/`extra`/`{state}.qemu.cmd` for command inspection/override. The ISO pipeline stays in
  the external `win11-install` → `win11-installer` bash. (`vmd clone` = still the `win11-clone` tool.)
- [x] **Phase 5** — Dockerfile multi-stage (static musl `vmd`); single `vmd` service (`ZSRV_vmd`);
  removed `vm/tpm/novnc` services, the `start-*`/`vm-console` scripts, and `websockify`/`socat`.

## Build

```bash
cargo build --release                                      # host build (~830K)
cargo build --release --target x86_64-unknown-linux-musl   # static; copy target/.../vmd to /build/bin/
```

> Phase 1 verified: `cargo build` (debug + release) is clean and `cargo clippy` reports no issues.
> Config loading + profile selection (`VMD_OS`) + arg building were smoke-tested end to end.
