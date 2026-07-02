#![forbid(unsafe_code)]

//! vmd — OS-agnostic QEMU supervisor. Guests are pure config (vmd.toml): vmd fills {placeholders},
//! runs install/seed/prepare/sidecars, spawns QEMU and drives the power lifecycle over QMP.
//!
//! Placeholders: {accel} {cpu} {cpus} {ram} {name} {uuid} {mac} {state} {dir} {disk} {disk_size}
//! {vnc_sock} {qmp} {console_sock} {tpm_sock} {web_port}. The QEMU command MUST include
//! `-qmp unix:{qmp},server,nowait`.
//!
//! Script resolution (launcher, install, …): {dir}/scripts/<slot> (user copy, wins) <- template
//! folder $VMD_TEMPLATES/<name>/ when `launch` is a bare name <- a custom path. Copies are seeded
//! once and never overwritten.
//!
//! Subcommands: `run` (default), `print` (dry run), `power <start|shutdown|reset|poweroff|status>`.

mod config;
mod install;
mod lifecycle;
mod log;
mod qmp;
mod seed;
mod sidecar;
mod subst;
mod util;
mod web;

use anyhow::{bail, Context, Result};
use config::Config;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use subst::{substitute, tokenize, Vars};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("run");
    let rest = &args[args.len().min(2)..];
    let result = match cmd {
        "run" => run().await,
        "print" | "dry-run" => print_plan(),
        "power" => power(rest).await,
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        other => Err(anyhow::anyhow!("unknown subcommand '{other}' (run, print, power)")),
    };
    if let Err(e) = result {
        log::error(format!("{e:#}"));
        std::process::exit(1);
    }
}

/// Prepare once, serve the web console, then loop: spawn QEMU → supervise → on clean power-off
/// idle until `Start`; container stop exits 0, a crash exits with QEMU's code.
async fn run() -> Result<()> {
    config::seed_storage_config(); // give /vms an editable vmd.toml on first run
    let cfg = Config::load()?;
    log::info(format!("starting guest '{}' disk={}", cfg.name, cfg.disk.display()));
    if let Some(dir) = &cfg.dir {
        tokio::fs::create_dir_all(dir).await.ok();
    }
    if let Some(parent) = cfg.disk.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }

    let stem = cfg.state_stem();
    let uuid = util::get_or_create_uuid(&stem)?;
    let vars = build_vars(&cfg, &uuid);

    // Copy the system's scripts into {dir}/scripts first, so install runs the user-editable copy
    // (and /info can scan the effective launcher for port forwards).
    ensure_scripts(&cfg).await?;

    // ---- resolve the launch command (also feeds /info) ----
    let qmp_sock = cfg.qmp_sock();
    let argv = resolve_command(&cfg, &vars);
    let Some((prog, args)) = argv.split_first() else {
        bail!("empty launch/qemu command for guest '{}'", cfg.name);
    };

    // ---- web console + power API (up before install so the install VM is watchable) ----
    let (ctrl_tx, mut ctrl_rx) = tokio::sync::mpsc::channel(8);
    // Mint a CSPRNG session secret only when auth is on; refuse to serve a password-protected console
    // if real entropy is unavailable (never fall back to a guessable token).
    let web_token = if cfg.web_password.is_empty() {
        log::info("web auth: disabled (no web.password set)");
        String::new()
    } else {
        log::info("web auth: password required");
        util::random_token().context("web session token")?
    };
    // Overrides /status while one-time setup runs (nobody is reading the control channel yet).
    let phase = std::sync::Arc::new(std::sync::RwLock::new(String::from("starting")));
    let web_state = web::WebState {
        vnc_sock: cfg.vnc_sock(),
        console_sock: cfg.console_sock(),
        ctrl: ctrl_tx,
        allowed_origins: cfg.web_allowed_origins.clone(),
        password: cfg.web_password.clone(),
        token: web_token,
        phase: phase.clone(),
        info: build_info(&cfg, &vars, &uuid, &argv),
    };
    // Bind here so a taken port is fatal instead of a silently console-less VM.
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", cfg.web_port))
        .await
        .with_context(|| format!("web bind :{}", cfg.web_port))?;
    tokio::spawn(web::serve(listener, web_state));

    // ---- one-time setup ----
    if let Some(inst) = &cfg.install {
        *phase.write().unwrap() = "installing".into();
        let script = effective_install(&cfg, inst)?;
        install::ensure(&cfg, inst, &script, &vars).await?; // external installer, gated by policy
        *phase.write().unwrap() = "starting".into();
    }
    for s in &cfg.seed {
        seed::ensure(&substitute(&s.template, &vars), &substitute(&s.to, &vars))?;
        // e.g. OVMF NVRAM
    }
    for p in &cfg.prepare {
        run_prepare(&substitute(p, &vars)).await?; // e.g. mkdir / tap setup
    }
    let mut items: Vec<(String, Option<String>)> = cfg
        .sidecars
        .iter()
        .map(|s| (substitute(&s.command, &vars), s.wait_for.as_ref().map(|w| substitute(w, &vars))))
        .collect();
    if cfg.tpm {
        let (cmd, sock) = tpm_command(&stem); // built-in vTPM: swtpm, ready before QEMU
        tokio::fs::create_dir_all(format!("{stem}.tpm")).await.ok();
        items.insert(0, (cmd, Some(sock)));
    }
    let mut sidecars = sidecar::Sidecars::start(&items, &vars).await?; // kept up across VM restarts
    phase.write().unwrap().clear(); // live status now comes from the supervisor

    // ---- boot / supervise / (idle & restart) loop ----
    loop {
        sidecars.ensure().await?; // respawn any that died with the last VM (swtpm does)
        let _ = tokio::fs::remove_file(&qmp_sock).await; // drop a stale socket
        log::info(argv.join(" "));
        let mut command = tokio::process::Command::new(prog);
        command.args(args).stdin(Stdio::null());
        if let Some(dir) = &cfg.dir {
            // keep QEMU's own output out of the supervisord log, in the guest's home dir
            let log = dir.join("qemu.log");
            let open = || std::fs::OpenOptions::new().create(true).append(true).open(&log);
            if let (Ok(out), Ok(err)) = (open(), open()) {
                command.stdout(Stdio::from(out)).stderr(Stdio::from(err));
            }
        }
        for (k, v) in &vars {
            command.env(format!("VMD_{}", k.to_uppercase()), v); // for external `launch` scripts
        }
        let child = command.spawn().with_context(|| format!("spawn {prog}"))?;

        match lifecycle::supervise(child, &qmp_sock, cfg.stop_grace, &mut ctrl_rx).await? {
            lifecycle::Outcome::Terminated => {
                sidecars.stop();
                std::process::exit(0);
            }
            lifecycle::Outcome::Crashed(code) => {
                sidecars.stop();
                std::process::exit(code);
            }
            lifecycle::Outcome::PoweredOff => {
                log::info("guest powered off — console still up; POST /power/start to boot again");
                match lifecycle::idle_until_start(&mut ctrl_rx).await? {
                    lifecycle::Idle::Terminated => {
                        sidecars.stop();
                        std::process::exit(0);
                    }
                    lifecycle::Idle::Start => log::info("starting the guest…"),
                }
            }
        }
    }
}

/// `vmd power <action>` — goes through the running vmd's local web API (it owns the lifecycle).
async fn power(args: &[String]) -> Result<()> {
    let cfg = Config::load()?;
    let action = args.first().map(String::as_str).unwrap_or("status");
    let (method, path) = match action {
        "start" | "shutdown" | "reset" | "poweroff" => ("POST", format!("/power/{action}")),
        "status" => ("GET", "/status".to_string()),
        other => bail!("unknown action '{other}' (start|shutdown|reset|poweroff|status)"),
    };
    let body = http_local(cfg.web_port, method, &path, &cfg.web_password).await?;
    log::info(format!("{action}: {body}"));
    Ok(())
}

/// Dependency-free HTTP/1.0 to the local web API; sends `X-VMD-Password` when auth is on.
async fn http_local(port: u16, method: &str, path: &str, password: &str) -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .with_context(|| format!("connect 127.0.0.1:{port} (is vmd running?)"))?;
    let auth = if password.is_empty() { String::new() } else { format!("X-VMD-Password: {password}\r\n") };
    let req = format!("{method} {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n{auth}Content-Length: 0\r\n\r\n");
    s.write_all(req.as_bytes()).await?;
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await?;
    let text = String::from_utf8_lossy(&buf);
    Ok(text.split("\r\n\r\n").nth(1).unwrap_or("").trim().to_string())
}

/// The command to run: the effective launcher, else the inline `qemu` template; plus `extra` args.
fn resolve_command(cfg: &Config, vars: &Vars) -> Vec<String> {
    let mut argv = match effective_launch(cfg) {
        Some(script) => vec![script.to_string_lossy().into_owned()],
        None => tokenize(&substitute(&cfg.qemu, vars)),
    };
    for e in &cfg.extra {
        argv.extend(tokenize(&substitute(e, vars)));
    }
    argv
}

/// Static VM facts for `/info`. Port forwards are scanned from the command, the launcher script
/// (hostfwd may live inside it) and `PORT_FWD`.
fn build_info(cfg: &Config, vars: &Vars, uuid: &str, argv: &[String]) -> web::VmInfo {
    let command = argv.join(" ");
    let launcher = effective_launch(cfg).and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let scan = format!("{command}\n{launcher}");
    let port_fwd_env = std::env::var("PORT_FWD").unwrap_or_default();
    let get = |k: &str| vars.get(k).cloned().unwrap_or_default();
    web::VmInfo {
        name: cfg.name.clone(),
        accel: get("accel"),
        cpu: get("cpu"),
        cpus: cfg.cpus,
        ram: cfg.ram.clone(),
        disk: cfg.disk.display().to_string(),
        disk_size: cfg.disk_size.clone(),
        uuid: uuid.to_string(),
        mac: get("mac"),
        tpm: cfg.tpm,
        web_port: cfg.web_port,
        port_forwards: web::collect_port_forwards(&scan, &port_fwd_env),
        command,
    }
}

/// Base dir of the built-in template folders (`$VMD_TEMPLATES/<name>/<slot>`).
fn templates_dir() -> String {
    std::env::var("VMD_TEMPLATES").unwrap_or_else(|_| "/build/templates".into())
}

/// Launcher source: bare name -> `$VMD_TEMPLATES/<name>/launcher`; a path -> itself.
fn launch_source(launch: &str) -> std::path::PathBuf {
    if launch.contains('/') {
        std::path::PathBuf::from(launch)
    } else {
        std::path::PathBuf::from(format!("{}/{launch}/launcher", templates_dir()))
    }
}

/// The launcher vmd runs: `{dir}/scripts/launcher` (user copy) when `dir` is set, else the source.
/// `None` = inline `qemu` command.
fn effective_launch(cfg: &Config) -> Option<std::path::PathBuf> {
    let launch = cfg.launch.as_deref()?;
    Some(match &cfg.dir {
        Some(dir) => dir.join("scripts").join("launcher"),
        None => launch_source(launch),
    })
}

/// Install script source: `install.launch` (bare name -> `<name>/install`, path -> itself), else
/// the guest's own template folder.
fn install_source(cfg: &Config, inst: &config::Install) -> Result<PathBuf> {
    match inst.launch.as_deref() {
        Some(p) if p.contains('/') => Ok(PathBuf::from(p)),
        Some(name) => Ok(PathBuf::from(format!("{}/{name}/install", templates_dir()))),
        None => {
            let launch = cfg
                .launch
                .as_deref()
                .filter(|l| !l.contains('/'))
                .context("install: set install.launch (the guest has no template to derive it from)")?;
            Ok(PathBuf::from(format!("{}/{launch}/install", templates_dir())))
        }
    }
}

/// The install script vmd runs: `{dir}/scripts/install` (user copy) when `dir` is set, else the source.
fn effective_install(cfg: &Config, inst: &config::Install) -> Result<PathBuf> {
    match &cfg.dir {
        Some(dir) => Ok(dir.join("scripts").join("install")),
        None => install_source(cfg, inst),
    }
}

/// Seed `{dir}/scripts/`: a bare `launch` name copies the whole template folder, a path copies that
/// one script to `scripts/launcher`. Existing files are never overwritten (user edits win).
async fn ensure_scripts(cfg: &Config) -> Result<()> {
    let (Some(dir), Some(launch)) = (cfg.dir.as_deref(), cfg.launch.as_deref()) else {
        return Ok(()); // no dir (run source in place) or inline qemu
    };
    let scripts = dir.join("scripts");
    tokio::fs::create_dir_all(&scripts).await.ok();
    if launch.contains('/') {
        seed_script(Path::new(launch), &scripts.join("launcher")).await?;
    } else {
        let folder = PathBuf::from(format!("{}/{launch}", templates_dir()));
        let mut entries = tokio::fs::read_dir(&folder)
            .await
            .with_context(|| format!("template folder not found: {}", folder.display()))?;
        while let Some(e) = entries.next_entry().await? {
            if e.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                seed_script(&e.path(), &scripts.join(e.file_name())).await?;
            }
        }
    }
    // an explicit install.launch selects a different source for the install slot
    if let Some(inst) = &cfg.install {
        if inst.launch.is_some() {
            seed_script(&install_source(cfg, inst)?, &scripts.join("install")).await?;
        }
    }
    Ok(())
}

/// Built-in vTPM 2.0 (`tpm = true`): the swtpm sidecar `(command, ready-socket)`. The launcher wires
/// the socket via `{tpm_sock}` / `$VMD_TPM_SOCK`. Pure — the caller creates the `{stem}.tpm` dir.
fn tpm_command(stem: &str) -> (String, String) {
    let dir = format!("{stem}.tpm");
    let sock = format!("{dir}/swtpm-sock");
    let cmd = format!(
        "swtpm socket --tpmstate dir={dir} --ctrl type=unixio,path={sock} --tpm2 --pid file={dir}/swtpm.pid"
    );
    (cmd, sock)
}

/// Copy `src` -> `dest` only when `dest` is missing (preserves the source's mode, incl. +x).
async fn seed_script(src: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        return Ok(());
    }
    tokio::fs::copy(src, dest)
        .await
        .with_context(|| format!("copy script {} -> {}", src.display(), dest.display()))?;
    log::info(format!("script installed at {} (edit to customize)", dest.display()));
    Ok(())
}

/// `vmd print` — show the resolved plan without running anything.
fn print_plan() -> Result<()> {
    let cfg = Config::load()?;
    // dry run: no side effects — placeholder UUID if the state dir doesn't exist yet
    let uuid = util::get_or_create_uuid(&cfg.state_stem()).unwrap_or_else(|_| "<uuid>".into());
    let vars = build_vars(&cfg, &uuid);
    println!("# guest: {}", cfg.name);
    if let Some(i) = &cfg.install {
        let env: Vec<String> = install::env_of(i, &vars).iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!("install [{}]: {} {}", i.policy, effective_install(&cfg, i)?.display(), env.join(" "));
    }
    for s in &cfg.seed {
        println!("seed: {} <- {}", substitute(&s.to, &vars), substitute(&s.template, &vars));
    }
    for p in &cfg.prepare {
        println!("prepare: {}", substitute(p, &vars));
    }
    if cfg.tpm {
        println!("sidecar: {} (built-in vTPM)", tpm_command(&cfg.state_stem()).0);
    }
    for s in &cfg.sidecars {
        println!("sidecar: {}", substitute(&s.command, &vars));
    }
    if let Some(l) = &cfg.launch {
        let src = launch_source(l);
        match effective_launch(&cfg) {
            Some(eff) if eff != src => println!("launch: {} (from {})", eff.display(), src.display()),
            Some(eff) => println!("launch: {}", eff.display()),
            None => {}
        }
    }
    println!("\nqemu: {}", resolve_command(&cfg, &vars).join(" "));
    Ok(())
}

/// The fixed, OS-agnostic placeholder set vmd provides to the config's command templates.
fn build_vars(cfg: &Config, uuid: &str) -> Vars {
    let kvm = Path::new("/dev/kvm").exists();
    if !kvm {
        log::warn("/dev/kvm unavailable -> slow TCG emulation (run with --device=/dev/kvm)");
    }
    let (accel, cpu) = if kvm { ("kvm", "host") } else { ("tcg", "max") };
    let mut v = Vars::new();
    v.insert("accel", accel.into());
    v.insert("cpu", cpu.into());
    v.insert("cpus", cfg.cpus.to_string());
    v.insert("ram", cfg.ram.clone());
    v.insert("name", cfg.name.clone());
    v.insert("uuid", uuid.into());
    v.insert("mac", util::gen_mac(&cfg.disk, 0));
    v.insert("state", cfg.state_stem());
    v.insert("dir", cfg.dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default());
    v.insert("disk", cfg.disk.display().to_string());
    v.insert("disk_size", cfg.disk_size.clone());
    v.insert("vnc_sock", cfg.vnc_sock().display().to_string());
    v.insert("qmp", cfg.qmp_sock().display().to_string());
    v.insert("console_sock", cfg.console_sock().display().to_string());
    v.insert("tpm_sock", format!("{}.tpm/swtpm-sock", cfg.state_stem()));
    v.insert("web_port", cfg.web_port.to_string());
    v
}

async fn run_prepare(cmd: &str) -> Result<()> {
    let toks = tokenize(cmd);
    let Some((prog, args)) = toks.split_first() else {
        return Ok(());
    };
    log::info(format!("prepare: {cmd}"));
    let status = tokio::process::Command::new(prog)
        .args(args)
        .status()
        .await
        .with_context(|| format!("prepare: {cmd}"))?;
    if !status.success() {
        bail!("prepare failed ({status}): {cmd}");
    }
    Ok(())
}

fn print_help() {
    println!("vmd — generic QEMU VM supervisor\n\nUSAGE:\n  vmd run              prepare + boot the active guest (per vmd.toml)\n  vmd print            show the resolved plan + QEMU command (dry run)\n  vmd power <action>   shutdown | reset | poweroff | status\n");
}
