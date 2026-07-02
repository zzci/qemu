//! Seed per-VM files: copy `template` -> `to` when `to` is missing (e.g. OVMF NVRAM on first boot).
//! Generic; vmd has no idea what the file is.

use crate::log;
use anyhow::{Context, Result};
use std::path::Path;

pub fn ensure(template: &str, to: &str) -> Result<()> {
    if Path::new(to).exists() {
        return Ok(());
    }
    if let Some(parent) = Path::new(to).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    log::info(format!("seed {to} <- {template}"));
    std::fs::copy(template, to).with_context(|| format!("seeding {to} from {template}"))?;
    Ok(())
}
