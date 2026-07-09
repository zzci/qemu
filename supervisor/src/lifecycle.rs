//! QEMU supervision + power lifecycle: react to child exit, SIGTERM/SIGINT (ACPI then SIGKILL
//! after grace), web power commands and QMP events; return an [`Outcome`] so `run` can loop.

use crate::log;
use crate::qmp::Qmp;
use anyhow::Result;
use std::path::Path;
use std::time::Duration;
use tokio::process::Child;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

/// Commands the web API sends to the VM.
pub enum Control {
    /// (Re)start the VM — only meaningful while it is off.
    Start,
    /// ACPI soft shutdown (the guest OS powers off).
    Shutdown,
    /// Hard reset (the reset button).
    Reset,
    /// Hard power-off — quit QEMU immediately, no ACPI.
    PowerOff,
    /// Save RAM + device state to the vmstate file, then stop QEMU (the next Start resumes).
    Save,
    /// Query run state; reply on the channel.
    Status(oneshot::Sender<String>),
}

pub type ControlTx = mpsc::Sender<Control>;
pub type ControlRx = mpsc::Receiver<Control>;

/// How a supervised QEMU ended.
pub enum Outcome {
    /// Clean guest power-off — stay off (web up) until a `Start`.
    PoweredOff,
    /// State saved to disk — stay off (web up); the next `Start` resumes from the snapshot.
    Saved,
    /// Container/operator stop (SIGTERM/SIGINT) — exit 0.
    Terminated,
    /// QEMU crashed — exit with its code so supervisord restarts vmd.
    Crashed(i32),
}

/// Supervise a freshly-spawned QEMU until it exits. `vmstate` is the standalone RAM/device-state
/// file (managedsave-style): `Save` produces it; when `resuming`, QEMU was launched with
/// `-incoming` reading it, and the file is consumed here right after QMP comes up (a broken
/// state file therefore cannot cause a resume loop — the retry boots cold).
pub async fn supervise(
    mut child: Child,
    qmp_path: &Path,
    grace: Duration,
    ctrl: &mut ControlRx,
    vmstate: &Path,
    resuming: bool,
) -> Result<Outcome> {
    // Register the signal handlers BEFORE the QMP handshake: a SIGTERM during the connect window
    // must not hit the default disposition (vmd would die and orphan QEMU without an ACPI stop).
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let (qmp, mut events) = tokio::select! {
        r = Qmp::connect(qmp_path) => match r {
            Ok(x) => x,
            // QEMU is up but uncontrollable (launcher without -qmp?) — kill it rather than exit
            // and leave an orphan holding the disk while supervisord respawns us.
            Err(e) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(e);
            }
        },
        status = child.wait() => {
            let status = status?;
            let code = status.code().unwrap_or_else(|| 128 + signal_of(&status));
            log::warn(format!("qemu exited before QMP came up (rc={code})"));
            // Even rc=0 here is a failed boot, not a guest power-off — report a crash.
            return Ok(Outcome::Crashed(if code == 0 { 1 } else { code }));
        }
        _ = sigterm.recv() => return kill_before_qmp(child).await,
        _ = sigint.recv() => return kill_before_qmp(child).await,
    };

    // Resuming: QEMU's `-incoming exec:cat …` already holds the file open and auto-continues the
    // guest when the stream ends; unlink it now so the slot is consumed exactly once.
    if resuming {
        log::info("restoring saved state (incoming migration)…");
        let _ = std::fs::remove_file(vmstate);
    }

    let mut kill_deadline: Option<Instant> = None;
    let mut term = false;
    let mut saved = false;
    let mut events_open = true;
    let mut wake_retried = false;
    // Re-press the power button while shutdown is pending: Windows drops the event if the logon
    // UI isn't ready yet; repeats are harmless.
    let mut next_press: Option<Instant> = None;

    let code = loop {
        let until_deadline = async {
            match kill_deadline {
                Some(d) => tokio::time::sleep_until(d).await,
                None => std::future::pending::<()>().await,
            }
        };
        let until_press = async {
            match next_press {
                Some(d) => tokio::time::sleep_until(d).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            status = child.wait() => {
                let status = status?;
                break status.code().unwrap_or_else(|| 128 + signal_of(&status));
            }
            _ = sigterm.recv(), if kill_deadline.is_none() => {
                term = true;
                begin_powerdown(&qmp, grace, &mut kill_deadline, &mut next_press).await;
            }
            _ = sigint.recv(), if kill_deadline.is_none() => {
                term = true;
                begin_powerdown(&qmp, grace, &mut kill_deadline, &mut next_press).await;
            }
            cmd = ctrl.recv() => {
                match cmd {
                    Some(Control::Shutdown) => begin_powerdown(&qmp, grace, &mut kill_deadline, &mut next_press).await,
                    Some(Control::Reset)    => { log::info("hard reset (system_reset)"); let _ = qmp.reset().await; }
                    Some(Control::PowerOff) => { log::info("hard power-off (quit)"); let _ = qmp.quit().await; }
                    Some(Control::Save)     => {
                        // Blocks this loop for the duration (status queries queue up), which is
                        // acceptable for a rare operation.
                        match save_state(&qmp, vmstate).await {
                            Ok(()) => {
                                saved = true;
                                log::info("state saved — stopping QEMU (POST /power/start resumes)");
                                let _ = qmp.quit().await;
                            }
                            Err(e) => {
                                log::warn(format!("save failed: {e:#} — VM keeps running"));
                                let _ = std::fs::remove_file(vmstate); // drop a partial file
                                let _ = qmp.cont().await; // un-freeze if we got as far as stop
                            }
                        }
                    }
                    Some(Control::Start)    => { /* already running */ }
                    Some(Control::Status(reply)) => {
                        let s = qmp.status().await.unwrap_or_else(|_| "unknown".into());
                        let _ = reply.send(s);
                    }
                    None => {} // sender is held by the web task for the process lifetime
                }
            }
            ev = events.recv(), if events_open => {
                match ev {
                    // Guest slept instead of shutting down (Windows lock screen): wake + retry
                    // once, then force off (disks are flushed before S3, so quit is safe).
                    Some(e) if e == "SUSPEND" && kill_deadline.is_some() => {
                        if !wake_retried {
                            wake_retried = true;
                            log::warn("guest suspended instead of shutting down — waking + retrying powerdown");
                            let _ = qmp.wakeup().await;
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            let _ = qmp.powerdown().await;
                        } else {
                            log::warn("guest suspended again — forcing power-off (quit)");
                            let _ = qmp.quit().await;
                        }
                    }
                    Some(_) => {} // other events are just logged by the QMP layer
                    None => events_open = false, // QMP gone; child.wait() will resolve shortly
                }
            }
            _ = until_press, if next_press.is_some() => {
                log::info("guest still up — pressing the power button again");
                let _ = qmp.powerdown().await;
                next_press = Some(Instant::now() + Duration::from_secs(20));
            }
            _ = until_deadline, if kill_deadline.is_some() => {
                log::warn("grace period expired — SIGKILL");
                let _ = child.start_kill();
                kill_deadline = None;
                next_press = None;
            }
        }
    };

    Ok(if term {
        log::info(format!("container stopping (qemu rc={code})"));
        Outcome::Terminated
    } else if saved && code == 0 {
        Outcome::Saved
    } else if code == 0 {
        Outcome::PoweredOff
    } else {
        log::warn(format!("qemu exited abnormally (rc={code})"));
        Outcome::Crashed(code)
    })
}

/// Result of waiting while the VM is off.
pub enum Idle {
    /// A `Start` command arrived — re-spawn QEMU.
    Start,
    /// Container/operator stop — exit 0.
    Terminated,
}

/// VM off: keep the web console responsive until `Start` or container stop. `saved` switches the
/// reported status so the UI can tell "saved (start resumes)" from a plain power-off.
pub async fn idle_until_start(ctrl: &mut ControlRx, saved: bool) -> Result<Idle> {
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let status = if saved { "saved" } else { "off" };
    loop {
        tokio::select! {
            _ = sigterm.recv() => return Ok(Idle::Terminated),
            _ = sigint.recv() => return Ok(Idle::Terminated),
            cmd = ctrl.recv() => match cmd {
                Some(Control::Start) => return Ok(Idle::Start),
                Some(Control::Status(reply)) => { let _ = reply.send(status.into()); }
                Some(_) => {} // shutdown/reset/poweroff/save while already off: ignore
                None => return Ok(Idle::Terminated),
            }
        }
    }
}

/// Managedsave-style state dump: freeze the vCPUs, stream RAM + device state to the standalone
/// `vmstate` file over `migrate exec:`, and poll until done. The disk needs no snapshot — QEMU
/// quits right after, so disk content matches the frozen moment exactly.
async fn save_state(qmp: &Qmp, vmstate: &Path) -> anyhow::Result<()> {
    log::info(format!("saving VM state -> {} (takes a moment per GB of RAM)…", vmstate.display()));
    qmp.stop().await?;
    qmp.migrate_to(&format!("exec:cat > '{}'", vmstate.display())).await?;
    loop {
        tokio::time::sleep(Duration::from_millis(300)).await;
        match qmp.migrate_status().await?.as_str() {
            "completed" => return Ok(()),
            "active" | "setup" | "none" => {} // still streaming
            other => anyhow::bail!("migration {other}"),
        }
    }
}

/// Container stop before QMP was reachable: no ACPI path exists yet, so kill QEMU and reap it.
/// The guest booted seconds ago, so a hard stop is safe in practice.
async fn kill_before_qmp(mut child: Child) -> Result<Outcome> {
    log::warn("stop requested before QMP came up — killing QEMU");
    let _ = child.start_kill();
    let _ = child.wait().await;
    Ok(Outcome::Terminated)
}

/// ACPI power off (waking a sleeping guest first); arm the SIGKILL deadline + button re-press.
async fn begin_powerdown(
    qmp: &Qmp,
    grace: Duration,
    deadline: &mut Option<Instant>,
    next_press: &mut Option<Instant>,
) {
    if matches!(qmp.status().await.as_deref(), Ok("suspended")) {
        log::info("guest is suspended — waking it before powerdown");
        let _ = qmp.wakeup().await;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    log::info("ACPI powerdown…");
    let _ = qmp.powerdown().await;
    *deadline = Some(Instant::now() + grace);
    *next_press = Some(Instant::now() + Duration::from_secs(20));
}

fn signal_of(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.signal().unwrap_or(0)
}
