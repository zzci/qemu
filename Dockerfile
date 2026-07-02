# ---- web console (React + noVNC + xterm.js); dist is embedded into vmd below ----
FROM node:22-alpine AS ui
WORKDIR /w
COPY supervisor/ui/package.json supervisor/ui/package-lock.json* ./
RUN npm ci
COPY supervisor/ui/ ./
RUN npm run build   # -> /w/dist

# ---- vmd: static musl binary; UI embedded at compile time via include_dir!(ui/dist) ----
FROM rust:alpine AS vmd
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY supervisor /src
COPY --from=ui /w/dist /src/ui/dist
RUN cargo build --release   # profile strips + LTOs; output: target/release/vmd (UI baked in)

# ------------------------------------------------------------------- runtime ----
FROM zzci/ubase

# QEMU/KVM + OVMF + swtpm + ISO build tools; VNC/console/web are all vmd (no websockify/socat).
RUN apt-get update && apt-get install -y --no-install-recommends \
        qemu-system-x86 qemu-utils ovmf swtpm \
        p7zip-full xorriso wimtools \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# vmd (web console embedded) + Windows install pipeline + supervisord service.
COPY --from=vmd /src/target/release/vmd /build/bin/vmd
ADD rootfs /
COPY supervisor/vmd.toml /etc/vmd/vmd.toml
RUN chmod -R 0755 /build/bin /build/templates \
    && chmod 0644 /build/templates/qemu/common.sh \
    && find /build/config -type f -exec chmod 0644 {} +

# Single service `vmd`, toggled by ZSRV_vmd=true. Config: /vms/vmd.toml (or VMD_CONFIG), seeded
# from /etc/vmd/vmd.toml on first run; guest: VMD_OS. Example:
#   docker run -e ZSRV_vmd=true -p 8006:8006 -v "$PWD/vms:/vms" -v "$PWD/images:/images:ro" zzci/qemu
EXPOSE 8006
VOLUME ["/vms"]
# CMD ["/start.sh"] inherited from zzci/ubase (tini + supervisord)
