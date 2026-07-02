# Contributing

## Add a guest OS

A new guest is a `start-<os>` script (sharing the `qemu-common.sh` helpers) plus a docs page. The full how-to — starter
skeleton, `start-vm` wiring and conventions — lives in
[common/engine.md → Adding a guest](./common/engine.md#adding-a-guest) (not repeated here). When
done, document it from [`guests/_template.md`](./guests/_template.md) (+ `.zh-CN.md`), list it in
[README.md](./README.md), and add a commented service example to `docker-compose.yml`.

## Script naming (`rootfs/build/bin/`)

- `start-<thing>` — **launch scripts only** (what supervisord / `start-vm` exec):
  `start-vm`, `start-win11`, `start-alpine`, `start-novnc`, `start-tpm`.
- `<os>-<verb>` — per-OS operations run on demand: `win11-installer`, `win11-clone`.
- `vm-<verb>` — generic, guest-agnostic operations run on demand: `vm-console`.
- `qemu-common.sh` — generic, **sourced** QEMU helpers (logging, KVM/firmware/VNC detection, net /
  display / USB / serial / console arg builders, graceful-shutdown supervision, `resolve_disk`). No
  guest-specific logic lives here.

## Docs

- Bilingual, **two files** per page: `name.md` (English) + `name.zh-CN.md` (Chinese), cross-linked
  with the `English · 中文` toggle on line 3. No interleaving.
- OS-agnostic guides live in `docs/common/`, per-guest guides in `docs/guests/`.

## Commits

- English, conventional-commit style (`feat:`, `fix:`, `docs:`, `ci:`, `chore:`, …).
- Keep remote-visible text (commit messages, PR titles) English.

## Build & sanity-check

```bash
docker build -t zzci/qemu .
bash -n rootfs/build/bin/*           # shell syntax
docker compose config -q             # compose syntax
```
