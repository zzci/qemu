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

    match inst.policy.as_str() {
        "none" => {
            if installed.exists() {
                return Ok(());
            }
            if cfg.disk.exists() {
                std::fs::write(&installed, "migrated\n").ok();
                return Ok(());
            }
            bail!("no installed disk and install.policy='none' — set policy='auto' (or 'force')");
        }
        "auto" => {
            if installed.exists() {
                return Ok(());
            }
            run(inst, script, vars, false).await?;
            std::fs::write(&installed, "installed\n").ok();
            Ok(())
        }
        "force" => {
            if force_applied.exists() {
                log::info("install.policy=force already applied -> booting");
                if !installed.exists() {
                    run(inst, script, vars, false).await?;
                    std::fs::write(&installed, "installed\n").ok();
                }
                return Ok(());
            }
            log::info("install.policy=force: wipe + reinstall (one-shot)");
            run(inst, script, vars, true).await?;
            std::fs::write(&installed, "installed\n").ok();
            std::fs::write(&force_applied, "1\n").ok();
            Ok(())
        }
        other => bail!("unknown install.policy '{other}' (auto|force|none)"),
    }
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
    let status = command
        .status()
        .await
        .with_context(|| format!("running installer: {}", script.display()))?;
    if !status.success() {
        bail!("installer failed ({status}): {}", script.display());
    }
    Ok(())
}
