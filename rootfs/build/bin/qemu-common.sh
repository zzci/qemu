#!/usr/bin/env bash
# shellcheck shell=bash
# qemu-common.sh — shared helpers for the QEMU guest scripts. Sourced (not executed) by
# launchers and installers. Set LOG_TAG before use.
#
# Contract — these helpers read/write globals the caller is expected to provide:
#   log/die          read  LOG_TAG
#   detect_kvm       set   ACCEL_MODE (kvm|tcg), CPU_MODEL (host|max)
#   setup_firmware   read  VARS; set CODE; seed VARS (OVMF NVRAM)
#   build_vnc_args   read  VNC_HOST, VNC_PASSWORD, VNC_SECRET; set VNC_ARGS
#   gen_mac          read  DISK
#   supervise_qemu   read  MONITOR; arg $1 = qemu pid; exits with QEMU's status

# Colored logging. LOG_TAG is the bracketed prefix (win11 | alpine | installer | tpm | vm | clone).
log() { printf '\033[1;34m[%s]\033[0m %s\n' "${LOG_TAG:-vm}" "$*"; }
die() { printf '\033[1;31m[%s] !! %s\033[0m\n' "${LOG_TAG:-vm}" "$*" >&2; exit 1; }

# KVM detection -> ACCEL_MODE / CPU_MODEL. Array-based callers build their own
# -machine/-cpu from these; start-win11 feeds ACCEL_MODE into its -readconfig file.
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

# QEMU user-mode (SLIRP) host->guest forwards from a "host-guest,host-guest" list ($1). Echoes the
# ",hostfwd=tcp::H-:G" segments to append to -netdev user,id=... ; empty list = no forwards.
user_hostfwd() {
    local pair h g out=""; local IFS=','
    for pair in $1; do
        [ -n "$pair" ] || continue
        h="${pair%-*}"; g="${pair#*-}"
        out="${out},hostfwd=tcp::${h}-:${g}"
    done
    printf '%s' "$out"
}

# VNC endpoint for the noVNC bridge -> VNC_ARGS. VNC_HOST sets the bind address (localhost = console
# only via the bridge; 0.0.0.0 exposes it). VNC_PASSWORD (empty = none) is passed via a secret file
# so it never hits the command line; the VNC protocol truncates it to 8 chars.
build_vnc_args() {
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

# Stable, locally-administered guest MAC derived from the disk path (so a clone/restart keeps its
# DHCP lease). Optional $1 indexes additional NICs.
gen_mac() { printf '52:54:00:%s' "$(printf '%s%s' "$DISK" "${1:-0}" | md5sum | sed 's/\(..\)\(..\)\(..\).*/\1:\2:\3/')"; }

# Default DISK to the per-OS path (must match each guest starter's default) when unset, so
# cross-cutting helpers (start-tpm, vm-console) derive the same per-VM paths as the running starter.
resolve_disk() {
    : "${STORAGE:=/vms}"
    local os="${OS:-win11}"
    case "${os,,}" in
        win11|windows|win) : "${DISK:=$STORAGE/win11/windows.qcow2}" ;;
        *)                 : "${DISK:=$STORAGE/${os,,}/${os,,}.qcow2}" ;;
    esac
}

# Display adapter (VGA) + optional forced resolution (RESOLUTION, via EDID) -> DISPLAY_ARGS.
build_display_args() {
    local w h edid=""
    if [ -n "${RESOLUTION:-}" ]; then w="${RESOLUTION%x*}"; h="${RESOLUTION#*x}"; edid="edid=on,xres=$w,yres=$h"; fi
    case "${VGA:-std}" in
        virtio) DISPLAY_ARGS=(-device "virtio-vga${edid:+,$edid}") ;;
        qxl)    DISPLAY_ARGS=(-device "qxl-vga${edid:+,$edid}") ;;
        std|*)  DISPLAY_ARGS=(-device "VGA${edid:+,$edid}") ;;
    esac
    [ -n "${RESOLUTION:-}" ] && log "display: $VGA @ ${RESOLUTION}" || log "display: ${VGA:-std} (guest-chosen resolution)"
}

# Networking by mode (NETWORK = user|bridge|host|macvlan|none) -> NET_ARGS. user mode appends
# PORT_FWD forwards; bridge/host enslave a tap to $BRIDGE; macvlan opens macvtap fds on $MACVLAN.
build_net_args() {
    NET_ARGS=(); local n=0 dev mac tap idx maj min fd=30
    case "$NETWORK" in
        none) NET_ARGS=(-nic none) ;;
        user)
            NET_ARGS=( -netdev "user,id=net0$(user_hostfwd "$PORT_FWD")"
                       -device "virtio-net-pci,netdev=net0,mac=$(gen_mac 0)" ) ;;
        bridge|host)
            [ -n "$BRIDGE" ] || die "NETWORK=$NETWORK needs BRIDGE=<existing bridge> (and --cap-add NET_ADMIN, /dev/net/tun)"
            tap="qtap0"; mac=$(gen_mac 0)
            log "tap '$tap' on bridge '$BRIDGE' -> guest NIC net0 ($mac)"
            ip tuntap add dev "$tap" mode tap 2>/dev/null || true
            ip link set "$tap" master "$BRIDGE" || die "cannot enslave $tap to $BRIDGE"
            ip link set "$tap" up
            NET_ARGS=( -netdev "tap,id=net0,ifname=$tap,script=no,downscript=no"
                       -device "virtio-net-pci,netdev=net0,mac=$mac" ) ;;
        macvlan)
            [ -n "$MACVLAN" ] || die "NETWORK=macvlan needs MACVLAN=<container iface> + a docker 'macvlan' network"
            local IFS=','; for dev in $MACVLAN; do
                [ -n "$dev" ] || continue
                [ -d "/sys/class/net/$dev" ] || die "MACVLAN iface '$dev' not present"
                mac=$(gen_mac "$n"); tap="mvtap${n}"
                log "macvtap '$tap' on '$dev' -> guest NIC net${n} ($mac), LAN IP via DHCP"
                ip link del "$tap" 2>/dev/null || true
                ip link add link "$dev" name "$tap" address "$mac" type macvtap mode bridge \
                    || die "macvtap failed (need --cap-add NET_ADMIN + 'macvlan' network)"
                ip link set "$tap" up
                idx=$(cat "/sys/class/net/$tap/ifindex")
                read -r maj min < <(tr ':' ' ' < /sys/devices/virtual/net/"$tap"/tap*/dev)
                [ -e "/dev/tap$idx" ] || mknod "/dev/tap$idx" c "$maj" "$min"
                eval "exec ${fd}<>/dev/tap$idx"
                NET_ARGS+=( -netdev "tap,id=net${n},fd=${fd}" -device "virtio-net-pci,netdev=net${n},mac=$mac" )
                n=$((n+1)); fd=$((fd+1))
            done ;;
        *) die "unknown NETWORK=$NETWORK (user|bridge|macvlan|host|none)" ;;
    esac
    return 0
}

# USB passthrough from USB ("vendor:product[,vendor:product]" hex) -> USB_ARGS.
build_usb_args() {
    USB_ARGS=(); local item v p
    local IFS=','; for item in ${USB:-}; do
        [ -n "$item" ] || continue
        v="${item%%:*}"; p="${item##*:}"
        log "usb passthrough: vendor=$v product=$p (host device must be visible in the container)"
        USB_ARGS+=( -device "usb-host,vendorid=0x${v},productid=0x${p}" )
    done
    return 0
}

# Host serial ports -> guest COM ports. SERIAL = comma list of host TTY paths (e.g.
# /dev/ttyUSB0,/dev/ttyS0); each becomes a guest pci-serial COM port (ser0, ser1, …). USB-serial
# adapters are better passed via USB; socket/telnet serial stays in EXTRA_ARGS. -> SERIAL_ARGS.
build_serial_args() {
    SERIAL_ARGS=(); local n=0 dev
    local IFS=','; for dev in ${SERIAL:-}; do
        [ -n "$dev" ] || continue
        [ -e "$dev" ] || log "WARNING: serial '$dev' not present in the container (add --device=$dev)"
        log "serial: host $dev -> guest COM (pci-serial ser${n})"
        SERIAL_ARGS+=( -chardev "serial,id=ser${n},path=$dev" -device "pci-serial,chardev=ser${n}" )
        n=$((n+1))
    done
    return 0
}

# Optional text/serial console: CONSOLE=on wires the guest's primary serial (ttyS0 / COM1) to a
# per-VM unix socket, which `vm-console` attaches an interactive terminal to. Mainly for Linux
# guests (they need `console=ttyS0` on the kernel cmdline for a login/boot console). -> CONSOLE_ARGS.
build_console_args() {
    CONSOLE_ARGS=()
    [ "${CONSOLE:-off}" = on ] || return 0
    local sock="${DISK%.*}.console.sock"
    rm -f "$sock"
    log "serial console on $sock (attach: docker exec -it <c> vm-console; Linux needs console=ttyS0)"
    CONSOLE_ARGS=( -serial "unix:$sock,server,nowait" )
}

# Supervise a running QEMU ($1 = pid) and exit with its status. On container TERM/INT, ask the guest
# to ACPI power off, then keep waiting until QEMU really exits (a trapped signal interrupts `wait`
# without the child having gone — re-wait, or QEMU is SIGKILLed mid-flush and the disk is left
# dirty). rc=0 (clean poweroff) is "expected" under supervisord autorestart=unexpected, so the VM
# stays off; a non-zero crash is restarted.
supervise_qemu() {
    local qpid="$1" rc=0
    TERMINATING=0
    trap 'TERMINATING=1; log "ACPI powerdown..."; printf "system_powerdown\n" | socat - "UNIX-CONNECT:$MONITOR" >/dev/null 2>&1 || true' TERM INT
    while kill -0 "$qpid" 2>/dev/null; do
        if wait "$qpid"; then rc=0; else rc=$?; fi
    done
    rm -f "$MONITOR"
    if [ "$TERMINATING" = 1 ]; then
        log "container stopping (qemu rc=$rc)"
    elif [ "$rc" -eq 0 ]; then
        log "guest powered off — staying off (run 'sctl start vm' or restart the container to boot again)"
    else
        log "qemu exited abnormally (rc=$rc) — supervisord will restart it"
    fi
    exit "$rc"
}
