# Contributing

## Add a guest OS

A guest is **config + a template folder** — no Rust changes:

1. Create `rootfs/build/templates/<name>/` with:
   - `launcher` — builds the QEMU command from `VMD_*` env vars and `exec`s it. It **must**
     include `-qmp "unix:${VMD_QMP},server,nowait"`. Start from `templates/alpine/launcher`
     (simple BIOS) or `templates/win11/launcher` (UEFI + TPM).
   - `install` (optional) — prepares the disk on first boot. Reads `VMD_DISK`/`VMD_DISK_SIZE`
     plus any `[guest.x.install]` keys as UPPERCASE env vars; `FORCE=1` means wipe and rebuild;
     exit 0 = installed.
2. Add a `[guest.<name>]` block to `supervisor/vmd.toml` (dir, disk, resources, `launch = "<name>"`,
   optional `[guest.<name>.install]`).
3. Sanity-check with `VMD_OS=<name> vmd print`, then boot it.
4. Document it from [`guests/_template.md`](./guests/_template.md) (+ `.zh-CN.md`) and list it in
   the READMEs.

Conventions: templates are copied to `{dir}/scripts/` on first boot and the copies are what runs —
keep them self-contained (shared helpers may be sourced from `/build/templates/qemu/common.sh`).

## Layout

- `supervisor/` — the vmd Rust crate; `supervisor/ui/` — the web console (React, embedded into the
  binary at build time).
- `rootfs/build/templates/<name>/` — per-guest scripts; `rootfs/build/templates/qemu/common.sh` —
  shared sourced helpers; `rootfs/build/bin/` — on-demand tools (`win11-clone`).
- `rootfs/build/config/` — e.g. `autounattend.xml.tmpl`.

## Docs

- Bilingual, **two files** per page: `name.md` (English) + `name.zh-CN.md` (Chinese), cross-linked
  with the `English · 中文` toggle. No interleaving.
- OS-agnostic guides live in `docs/common/`, per-guest guides in `docs/guests/`.

## Commits

- English, conventional-commit style (`feat:`, `fix:`, `docs:`, `ci:`, `chore:`, …).

## Build & sanity-check

```bash
docker build -t zzci/qemu .
( cd supervisor && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test )
( cd supervisor/ui && npm ci && npm run build )
shellcheck -x --severity=warning rootfs/build/bin/* rootfs/build/templates/*/install \
  rootfs/build/templates/*/launcher
docker compose config -q
```

CI (`.github/workflows/ci.yml`) runs the same gates on every push/PR.
