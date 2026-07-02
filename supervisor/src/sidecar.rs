//! Sidecars: programs/scripts run alongside the VM with the `VMD_<KEY>` env; optional `wait_for`
//! blocks until ready. They may die with the VM (QEMU shuts swtpm down), so the boot loop calls
//! [`Sidecars::ensure`] before every QEMU spawn.

use crate::log;
use crate::subst::{tokenize, Vars};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::process::{Child, Command};

struct Sidecar {
    command: String,
    wait_for: Option<String>,
    child: Child,
}

pub struct Sidecars {
    items: Vec<Sidecar>,
    env: Vec<(String, String)>,
}

impl Sidecars {
    /// `items`: (substituted command, optional ready-path); `vars` exported as `VMD_<KEY>`.
    pub async fn start(items: &[(String, Option<String>)], vars: &Vars) -> Result<Sidecars> {
        let env: Vec<(String, String)> =
            vars.iter().map(|(k, v)| (format!("VMD_{}", k.to_uppercase()), v.clone())).collect();
        let mut out = Sidecars { items: Vec::new(), env };
        for (cmd, wait_for) in items {
            if cmd.split_whitespace().next().is_none() {
                continue;
            }
            let child = out.spawn_one(cmd).await?;
            out.items.push(Sidecar { command: cmd.clone(), wait_for: wait_for.clone(), child });
            if let Some(path) = wait_for {
                wait_path(path).await.with_context(|| format!("sidecar not ready: {cmd}"))?;
            }
        }
        Ok(out)
    }

    /// Respawn exited sidecars (and re-wait their ready-path); called before each QEMU spawn.
    pub async fn ensure(&mut self) -> Result<()> {
        for i in 0..self.items.len() {
            if self.items[i].child.try_wait()?.is_none() {
                continue; // still running
            }
            let (cmd, wait_for) = (self.items[i].command.clone(), self.items[i].wait_for.clone());
            log::info(format!("sidecar exited — respawning: {cmd}"));
            if let Some(path) = &wait_for {
                let _ = std::fs::remove_file(path); // drop the stale ready-socket
            }
            self.items[i].child = self.spawn_one(&cmd).await?;
            if let Some(path) = &wait_for {
                wait_path(path).await.with_context(|| format!("sidecar not ready: {cmd}"))?;
            }
        }
        Ok(())
    }

    async fn spawn_one(&self, cmd: &str) -> Result<Child> {
        let toks = tokenize(cmd);
        let Some((prog, args)) = toks.split_first() else {
            bail!("empty sidecar command");
        };
        log::info(format!("sidecar: {cmd}"));
        let mut command = Command::new(prog);
        command.args(args).kill_on_drop(true);
        for (k, v) in &self.env {
            command.env(k, v);
        }
        command.spawn().with_context(|| format!("spawn sidecar: {cmd}"))
    }

    /// SIGKILL all sidecars (`kill_on_drop` backs error paths).
    pub fn stop(&mut self) {
        for s in &mut self.items {
            let _ = s.child.start_kill();
        }
    }
}

async fn wait_path(path: &str) -> Result<()> {
    for _ in 0..100 {
        if Path::new(path).exists() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    bail!("path never appeared: {path}")
}
