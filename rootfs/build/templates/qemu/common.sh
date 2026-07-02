#!/usr/bin/env bash
# shellcheck shell=bash
# templates/qemu/common.sh — shared helpers for the QEMU guest scripts. Sourced (not executed)
# by launchers and installers. Set LOG_TAG before use.
#
# Contract — these helpers read/write globals the caller is expected to provide:
#   log/die          read  LOG_TAG
#   detect_kvm       set   ACCEL_MODE (kvm|tcg), CPU_MODEL (host|max)
#   setup_firmware   read  VARS; set CODE; seed VARS (OVMF NVRAM)
#   build_vnc_args   read  VNC_SOCK | VNC_HOST, VNC_PASSWORD, VNC_SECRET; set VNC_ARGS

# Colored logging. LOG_TAG is the bracketed prefix (win11 | alpine | install | clone).
log() { printf '\033[1;34m[%s]\033[0m %s\n' "${LOG_TAG:-vm}" "$*"; }
die() { printf '\033[1;31m[%s] !! %s\033[0m\n' "${LOG_TAG:-vm}" "$*" >&2; exit 1; }

# KVM detection -> ACCEL_MODE / CPU_MODEL; callers build their own -machine/-cpu from these.
detect_kvm() {
    if [ -c /dev/kvm ] && qemu-system-x86_64 -accel help 2>/dev/null | grep -qw kvm; then
        log "KVM enabled (hardware accelerated)"
        ACCEL_MODE=kvm; CPU_MODEL=host
    else
        log "WARNING: /dev/kvm unavailable -> slow TCG emulation (run with --device=/dev/kvm)"
        ACCEL_MODE=tcg; CPU_MODEL=max
    fi
}

# OVMF firmware: pick the code blob (4M variant if present) into CODE, and seed a per-VM NVRAM
# copy at VARS on first boot. Prefers the matching 4M VARS so code/vars sizes agree.
setup_firmware() {
    CODE=/usr/share/OVMF/OVMF_CODE_4M.fd; [ -f "$CODE" ] || CODE=/usr/share/OVMF/OVMF_CODE.fd
    [ -f "$VARS" ] && return 0
    [ -f /usr/share/OVMF/OVMF_VARS_4M.fd ] && cp /usr/share/OVMF/OVMF_VARS_4M.fd "$VARS" \
        || cp /usr/share/OVMF/OVMF_VARS.fd "$VARS"
}

# VNC endpoint -> VNC_ARGS. VNC_SOCK (a unix socket path) wins: it is what the vmd web console's
# noVNC bridge dials, so an install VM is watchable at http://<host>:<web_port>/. Otherwise fall
# back to TCP on VNC_HOST (localhost = not reachable from outside). VNC_PASSWORD (empty = none)
# is passed via a secret file so it never hits the command line; VNC truncates it to 8 chars.
build_vnc_args() {
    if [ -n "${VNC_SOCK:-}" ]; then
        rm -f "$VNC_SOCK"
        VNC_ARGS=( -vnc "unix:${VNC_SOCK}" )
        log "VNC: unix socket $VNC_SOCK (watch via the web console)"
        return 0
    fi
    if [ -n "${VNC_PASSWORD:-}" ]; then
        printf '%s' "$VNC_PASSWORD" > "$VNC_SECRET"; chmod 600 "$VNC_SECRET"
        VNC_ARGS=( -object "secret,id=vncsec,file=$VNC_SECRET"
                   -vnc "${VNC_HOST}:0,password-secret=vncsec" )
        log "VNC: ${VNC_HOST}:5900 (password set; VNC uses only the first 8 chars)"
    else
        rm -f "$VNC_SECRET"
        VNC_ARGS=( -vnc "${VNC_HOST}:0" )
        log "VNC: ${VNC_HOST}:5900 (no password)"
    fi
}
