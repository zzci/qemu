//! Install orchestration — vmd keeps no install logic. It gates on `install.policy`, runs the
//! resolved install script with the config's options as UPPERCASE env vars (+ `VMD_<KEY>` + FORCE),
//! then records an `.installed` marker.

use crate::config::{Config, Install};
use crate::log;
use crate::subst::{substitute, Vars};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub async fn ensure(cfg: &Config, inst: &Install, script: &Path, vars: &Vars) -> Result<()> {
    let stem = cfg.state_stem();
    let installed = PathBuf::from(format!("{stem}.installed"));
    let force_applied = PathBuf::from(format!("{stem}.force-applied"));

    // The marker only counts when the disk is actually there — a deleted disk with a stale marker
    // would otherwise send QEMU into a boot-failure loop that only manual marker removal fixes.
    let marker_valid = installed.exists() && cfg.disk.exists();
    if installed.exists() && !cfg.disk.exists() {
        log::warn(format!(
            "install marker {} exists but disk {} is missing",
            installed.display(),
            cfg.disk.display()
        ));
    }

    match inst.policy.as_str() {
        "none" => {
            if marker_valid {
                return Ok(());
            }
            if cfg.disk.exists() {
                write_marker(&installed, "migrated\n")?;
                return Ok(());
            }
            bail!("no installed disk and install.policy='none' — set policy='auto' (or 'force')");
        }
        "auto" => {
            if marker_valid {
                return Ok(());
            }
            run(inst, script, vars, false).await?;
            write_marker(&installed, "installed\n")?;
            Ok(())
        }
        "force" => {
            if force_applied.exists() {
                log::info("install.policy=force already applied -> booting");
                if !marker_valid {
                    run(inst, script, vars, false).await?;
                    write_marker(&installed, "installed\n")?;
                }
                return Ok(());
            }
            log::info("install.policy=force: wipe + reinstall (one-shot)");
            run(inst, script, vars, true).await?;
            write_marker(&installed, "installed\n")?;
            write_marker(&force_applied, "1\n")?;
            Ok(())
        }
        other => bail!("unknown install.policy '{other}' (auto|force|none)"),
    }
}

/// Persist an install marker; failing to record it must be fatal, or the (possibly destructive)
/// installer would silently re-run on every boot.
fn write_marker(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents).with_context(|| format!("writing install marker {}", path.display()))
}

/// The install-table options as (UPPERCASE_KEY, substituted value) env pairs.
pub fn env_of(inst: &Install, vars: &Vars) -> Vec<(String, String)> {
    inst.env
        .iter()
        .map(|(k, v)| {
            let val = match v {
                toml::Value::String(s) => substitute(s, vars),
                other => other.to_string(),
            };
            (k.to_uppercase(), val)
        })
        .collect()
}

async fn run(inst: &Install, script: &Path, vars: &Vars, force: bool) -> Result<()> {
    log::info(format!("install: {} (force={force})", script.display()));
    let mut command = tokio::process::Command::new(script);
    command.env("FORCE", if force { "1" } else { "0" });
    for (k, v) in vars {
        command.env(format!("VMD_{}", k.to_uppercase()), v); // same VMD_<KEY> env as the launcher
    }
    for (k, v) in env_of(inst, vars) {
        command.env(k, v);
    }
    let status =
        command.status().await.with_context(|| format!("running installer: {}", script.display()))?;
    if !status.success() {
        bail!("installer failed ({status}): {}", script.display());
    }
    Ok(())
}
