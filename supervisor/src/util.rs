//! Small helpers: stable per-VM MAC, session token, persistent UUID.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Stable guest MAC from the disk path (FNV-1a), so restarts keep their DHCP lease.
pub fn gen_mac(disk: &Path, idx: u8) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let bytes = disk.as_os_str().to_string_lossy();
    for b in bytes.bytes().chain(std::iter::once(idx)) {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("52:54:00:{:02x}:{:02x}:{:02x}", (h >> 16) as u8, (h >> 8) as u8, h as u8)
}

/// Web session secret from the kernel CSPRNG. Never falls back to a constant (a predictable token
/// would let anyone forge the auth cookie) — on RNG failure the caller must refuse to serve.
pub fn random_token() -> Result<String> {
    let uuid = std::fs::read_to_string("/proc/sys/kernel/random/uuid")
        .context("reading kernel CSPRNG (/proc/sys/kernel/random/uuid) for the web session token")?;
    Ok(uuid.trim().replace('-', ""))
}

/// Persistent per-disk UUID — keeps SMBIOS identity stable across restarts (activation).
pub fn get_or_create_uuid(state_stem: &str) -> Result<String> {
    let path = PathBuf::from(format!("{state_stem}.uuid"));
    if let Ok(s) = std::fs::read_to_string(&path) {
        let s = s.trim().to_string();
        if !s.is_empty() {
            return Ok(s);
        }
    }
    let uuid = std::fs::read_to_string("/proc/sys/kernel/random/uuid")?.trim().to_string();
    std::fs::write(&path, format!("{uuid}\n"))?;
    Ok(uuid)
}
