//! QMP client: greeting/capabilities handshake, then a background reader matches responses to
//! callers in order and forwards notable events. `Qmp` is a cheap cloneable handle.

use crate::log;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};

struct Cmd {
    req: Value,
    reply: oneshot::Sender<Result<Value>>,
}

#[derive(Clone)]
pub struct Qmp {
    tx: mpsc::Sender<Cmd>,
}

impl Qmp {
    /// Connect (retrying until the socket exists), negotiate capabilities, start the background
    /// reader; also returns the notable-event stream (SHUTDOWN, SUSPEND, …).
    pub async fn connect(path: &Path) -> Result<(Qmp, mpsc::Receiver<String>)> {
        let stream = connect_retry(path).await?;
        let (read, write) = stream.into_split();
        let mut reader = BufReader::new(read);
        let mut writer = write;

        // greeting: {"QMP":{"version":{"qemu":{major,minor,micro},...},...}}
        let greeting = read_message(&mut reader).await.context("QMP greeting")?;
        log::info(format!("QMP connected ({})", qemu_version(&greeting)));

        // capabilities handshake (skip any events that arrive during it)
        write_json(&mut writer, &json!({ "execute": "qmp_capabilities" })).await?;
        loop {
            let msg = read_message(&mut reader).await?;
            if msg.get("return").is_some() {
                break;
            }
            if let Some(e) = msg.get("error") {
                bail!("qmp_capabilities: {e}");
            }
        }

        // pipe the socket through two cancel-safe tasks: reader -> lines channel -> dispatcher.
        let (line_tx, line_rx) = mpsc::channel::<Value>(64);
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>(16);
        let (event_tx, event_rx) = mpsc::channel::<String>(16);
        tokio::spawn(reader_task(reader, line_tx));
        tokio::spawn(dispatch_task(writer, cmd_rx, line_rx, event_tx));
        Ok((Qmp { tx: cmd_tx }, event_rx))
    }

    pub async fn execute(&self, command: &str, arguments: Value) -> Result<Value> {
        let req = if arguments.is_null() || arguments == json!({}) {
            json!({ "execute": command })
        } else {
            json!({ "execute": command, "arguments": arguments })
        };
        let (reply, rx) = oneshot::channel();
        self.tx.send(Cmd { req, reply }).await.map_err(|_| anyhow!("QMP connection closed"))?;
        rx.await.map_err(|_| anyhow!("QMP connection closed"))?
    }

    pub async fn powerdown(&self) -> Result<()> {
        self.execute("system_powerdown", json!({})).await.map(|_| ())
    }
    pub async fn reset(&self) -> Result<()> {
        self.execute("system_reset", json!({})).await.map(|_| ())
    }
    pub async fn wakeup(&self) -> Result<()> {
        self.execute("system_wakeup", json!({})).await.map(|_| ())
    }
    pub async fn quit(&self) -> Result<()> {
        self.execute("quit", json!({})).await.map(|_| ())
    }
    pub async fn status(&self) -> Result<String> {
        let v = self.execute("query-status", json!({})).await?;
        Ok(v.get("status").and_then(Value::as_str).unwrap_or("unknown").to_string())
    }
}

/// Line reader in its own task (`read_line` is not cancel-safe under `select!`).
async fn reader_task(mut reader: BufReader<OwnedReadHalf>, line_tx: mpsc::Sender<Value>) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break, // closed
            Ok(_) => {
                if let Ok(v) = serde_json::from_str::<Value>(line.trim_end()) {
                    if line_tx.send(v).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

/// Write commands, match responses to callers in order, dispatch events.
async fn dispatch_task(
    mut writer: OwnedWriteHalf,
    mut cmd_rx: mpsc::Receiver<Cmd>,
    mut line_rx: mpsc::Receiver<Value>,
    event_tx: mpsc::Sender<String>,
) {
    let mut pending: VecDeque<oneshot::Sender<Result<Value>>> = VecDeque::new();
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(cmd) => match write_json(&mut writer, &cmd.req).await {
                    Ok(()) => pending.push_back(cmd.reply),
                    Err(e) => { let _ = cmd.reply.send(Err(anyhow!("QMP write: {e}"))); }
                },
                None => break, // all handles dropped
            },
            line = line_rx.recv() => match line {
                Some(msg) => {
                    if msg.get("return").is_some() || msg.get("error").is_some() {
                        if let Some(reply) = pending.pop_front() {
                            let r = match msg.get("error") {
                                Some(e) => Err(anyhow!("QMP error: {e}")),
                                None => Ok(msg.get("return").cloned().unwrap_or(Value::Null)),
                            };
                            let _ = reply.send(r);
                        }
                    } else if let Some(ev) = msg.get("event").and_then(Value::as_str) {
                        if notable_event(ev) {
                            log::info(format!("QMP event: {ev}"));
                            let _ = event_tx.try_send(ev.to_string()); // full buffer: drop, log stands
                        }
                    }
                }
                None => break, // reader ended (socket closed)
            },
        }
    }
    for reply in pending.drain(..) {
        let _ = reply.send(Err(anyhow!("QMP connection closed")));
    }
}

/// Guest power/panic events worth logging + forwarding; chatty ones ignored.
fn notable_event(ev: &str) -> bool {
    matches!(
        ev,
        "SHUTDOWN"
            | "POWERDOWN"
            | "RESET"
            | "STOP"
            | "RESUME"
            | "SUSPEND"
            | "SUSPEND_DISK"
            | "GUEST_PANICKED"
            | "WAKEUP"
            | "WATCHDOG"
    )
}

async fn connect_retry(path: &Path) -> Result<UnixStream> {
    let mut last = None;
    for _ in 0..100 {
        match UnixStream::connect(path).await {
            Ok(s) => return Ok(s),
            Err(e) => {
                last = Some(e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    Err(last.unwrap()).with_context(|| format!("QMP socket never came up at {}", path.display()))
}

async fn read_message(reader: &mut BufReader<OwnedReadHalf>) -> Result<Value> {
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        bail!("QMP connection closed");
    }
    Ok(serde_json::from_str(line.trim_end())?)
}

async fn write_json(writer: &mut OwnedWriteHalf, v: &Value) -> Result<()> {
    let mut buf = serde_json::to_vec(v)?;
    buf.push(b'\n');
    writer.write_all(&buf).await?;
    writer.flush().await?;
    Ok(())
}

fn qemu_version(greeting: &Value) -> String {
    let q = greeting.pointer("/QMP/version/qemu");
    match q {
        Some(v) => format!(
            "QEMU {}.{}.{}",
            v.get("major").and_then(Value::as_i64).unwrap_or(0),
            v.get("minor").and_then(Value::as_i64).unwrap_or(0),
            v.get("micro").and_then(Value::as_i64).unwrap_or(0),
        ),
        None => "unknown version".to_string(),
    }
}
