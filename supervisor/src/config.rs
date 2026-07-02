//! OS-agnostic config: guests are pure data. Search order: $VMD_CONFIG -> /vms/vmd.toml ->
//! /etc/vmd/vmd.toml; active guest: $VMD_OS or `default`.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Deserialize)]
struct RawFile {
    default: Option<String>,
    stop_grace_secs: Option<u64>,
    #[serde(default)]
    web: RawWeb,
    #[serde(default)]
    guest: BTreeMap<String, RawGuest>,
}
#[derive(Deserialize, Default)]
struct RawWeb {
    port: Option<u16>,
    allowed_origins: Option<Vec<String>>,
    /// Web access password; empty/absent = no auth.
    password: Option<String>,
}

#[derive(Deserialize)]
struct RawGuest {
    /// Guest home dir: disk/state/logs/scripts live here; relative `disk` resolves against it.
    dir: Option<String>,
    /// Disk image; absolute as-is, relative under `dir`, omitted -> `{dir}/disk.qcow2`.
    disk: Option<String>,
    disk_size: Option<String>,
    ram: Option<String>,
    cpus: Option<u32>,
    /// Inline QEMU command (alternative to `launch`); MUST include `-qmp unix:{qmp},server,nowait`.
    qemu: Option<String>,
    /// Launcher: a bare template name or a script path; run with placeholders as `VMD_<KEY>` env.
    launch: Option<String>,
    /// Extra args appended to the command (placeholder-substituted).
    #[serde(default)]
    extra: Vec<String>,
    /// Built-in vTPM 2.0: vmd runs swtpm as a managed sidecar; launcher uses `$VMD_TPM_SOCK`.
    #[serde(default)]
    tpm: bool,
    #[serde(default)]
    sidecars: Vec<RawSidecar>,
    #[serde(default)]
    prepare: Vec<String>,
    #[serde(default)]
    seed: Vec<Seed>,
    install: Option<Install>,
}

/// Copy `template` -> `to` when missing (e.g. per-VM OVMF NVRAM).
#[derive(Deserialize, Clone)]
pub struct Seed {
    pub template: String,
    pub to: String,
}

/// Sidecar: a command string, or `{ command, wait_for }` (block until the path exists).
#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum RawSidecar {
    Cmd(String),
    Full { command: String, wait_for: Option<String> },
}

#[derive(Clone)]
pub struct Sidecar {
    pub command: String,
    pub wait_for: Option<String>,
}

impl From<&RawSidecar> for Sidecar {
    fn from(r: &RawSidecar) -> Self {
        match r {
            RawSidecar::Cmd(c) => Sidecar { command: c.clone(), wait_for: None },
            RawSidecar::Full { command, wait_for } => {
                Sidecar { command: command.clone(), wait_for: wait_for.clone() }
            }
        }
    }
}

/// `[guest.<name>.install]` — external install script, gated by `policy` (auto|force|none).
/// `launch` resolves like the guest launcher (bare template name -> `<name>/install`, a path ->
/// itself; omitted -> the guest's own template). Every OTHER key in the table becomes an UPPERCASE
/// env var for the script (values placeholder-substituted); disk/size come from the guest as
/// `VMD_DISK` / `VMD_DISK_SIZE`.
#[derive(Deserialize, Clone)]
pub struct Install {
    pub policy: String,
    pub launch: Option<String>,
    #[serde(flatten)]
    pub env: BTreeMap<String, toml::Value>,
}

pub struct Config {
    pub name: String,
    pub dir: Option<PathBuf>,
    pub disk: PathBuf,
    pub disk_size: String,
    pub ram: String,
    pub cpus: u32,
    pub qemu: String,
    pub launch: Option<String>,
    pub extra: Vec<String>,
    pub tpm: bool,
    pub sidecars: Vec<Sidecar>,
    pub prepare: Vec<String>,
    pub seed: Vec<Seed>,
    pub install: Option<Install>,
    pub web_port: u16,
    pub web_allowed_origins: Vec<String>,
    pub web_password: String,
    pub stop_grace: Duration,
}

impl Config {
    pub fn load() -> Result<Config> {
        let path = config_path()?;
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("reading config {}", path.display()))?;
        let file: RawFile =
            toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))?;

        let name = env::var("VMD_OS")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| file.default.clone())
            .context("no active guest: set `default` in the config or VMD_OS")?;
        let g = file.guest.get(&name).with_context(|| format!("no [guest.{name}] in the config"))?;
        let qemu = g.qemu.clone().unwrap_or_default();
        let launch = g.launch.clone().filter(|s| !s.trim().is_empty());
        if qemu.trim().is_empty() && launch.is_none() {
            bail!("[guest.{name}] needs either a `qemu` command or a `launch` script");
        }
        let dir = g.dir.clone().filter(|s| !s.trim().is_empty()).map(PathBuf::from);
        let disk = resolve_disk(g.disk.as_deref(), dir.as_deref(), &name)?;

        Ok(Config {
            name,
            dir,
            disk,
            disk_size: g.disk_size.clone().unwrap_or_else(|| "16G".into()),
            ram: g.ram.clone().unwrap_or_else(|| "2G".into()),
            cpus: g.cpus.unwrap_or(2),
            qemu,
            launch,
            extra: g.extra.clone(),
            tpm: g.tpm,
            sidecars: g.sidecars.iter().map(Sidecar::from).collect(),
            prepare: g.prepare.clone(),
            seed: g.seed.clone(),
            install: g.install.clone(),
            web_port: file.web.port.unwrap_or(8006),
            web_allowed_origins: file.web.allowed_origins.clone().unwrap_or_default(),
            web_password: file.web.password.clone().unwrap_or_default(),
            stop_grace: Duration::from_secs(file.stop_grace_secs.unwrap_or(150)),
        })
    }

    /// `/vms/win11/windows.qcow2` -> `/vms/win11/windows`.
    pub fn state_stem(&self) -> String {
        state_stem_of(&self.disk)
    }
    pub fn qmp_sock(&self) -> PathBuf {
        PathBuf::from(format!("{}.qmp.sock", self.state_stem()))
    }
    pub fn console_sock(&self) -> PathBuf {
        PathBuf::from(format!("{}.console.sock", self.state_stem()))
    }
    pub fn vnc_sock(&self) -> PathBuf {
        PathBuf::from(format!("{}.vnc.sock", self.state_stem()))
    }
}

/// Strip the disk's extension (file name only, so a dotted directory like `/vms/v1.2/` is safe)
/// to form the per-VM state prefix.
fn state_stem_of(disk: &Path) -> String {
    disk.with_extension("").to_string_lossy().into_owned()
}

/// Disk path: absolute wins; relative joins `dir`; omitted -> `{dir}/disk.qcow2`.
fn resolve_disk(disk: Option<&str>, dir: Option<&Path>, name: &str) -> Result<PathBuf> {
    match (disk, dir) {
        (Some(d), _) if Path::new(d).is_absolute() => Ok(PathBuf::from(d)),
        (Some(d), Some(dir)) => Ok(dir.join(d)),
        (Some(d), None) => Ok(PathBuf::from(d)),
        (None, Some(dir)) => Ok(dir.join("disk.qcow2")),
        (None, None) => bail!("[guest.{name}] needs a `disk` (or a `dir` to default it)"),
    }
}

/// Seed /vms/vmd.toml from the baked default when a /vms volume exists without a config,
/// so users edit the live copy (like {dir}/scripts). Skipped when VMD_CONFIG overrides.
pub fn seed_storage_config() {
    if env::var("VMD_CONFIG").map(|v| !v.is_empty()).unwrap_or(false) {
        return;
    }
    let dst = Path::new("/vms/vmd.toml");
    if Path::new("/vms").is_dir()
        && !dst.exists()
        && Path::new("/etc/vmd/vmd.toml").exists()
        && std::fs::copy("/etc/vmd/vmd.toml", dst).is_ok()
    {
        crate::log::info("seeded /vms/vmd.toml from /etc/vmd/vmd.toml (edit to customize)");
    }
}

fn config_path() -> Result<PathBuf> {
    if let Ok(p) = env::var("VMD_CONFIG") {
        if !p.is_empty() {
            return Ok(PathBuf::from(p));
        }
    }
    for cand in ["/vms/vmd.toml", "/etc/vmd/vmd.toml"] {
        let p = PathBuf::from(cand);
        if p.exists() {
            return Ok(p);
        }
    }
    bail!("no config file (set VMD_CONFIG, or provide /vms/vmd.toml or /etc/vmd/vmd.toml)")
}

#[cfg(test)]
mod tests {
    use super::{resolve_disk, state_stem_of};
    use std::path::Path;

    #[test]
    fn state_stem_strips_only_the_file_extension() {
        assert_eq!(state_stem_of(Path::new("/vms/win11/windows.qcow2")), "/vms/win11/windows");
        assert_eq!(state_stem_of(Path::new("/vms/win11/win.2024.qcow2")), "/vms/win11/win.2024");
    }

    #[test]
    fn state_stem_keeps_dotted_directories_intact() {
        // a dot in a parent directory must never truncate the state prefix
        assert_eq!(state_stem_of(Path::new("/vms/v1.2/windows")), "/vms/v1.2/windows");
        assert_eq!(state_stem_of(Path::new("/vms/v1.2/windows.qcow2")), "/vms/v1.2/windows");
    }

    #[test]
    fn disk_resolution_rules() {
        let dir = Path::new("/vms/g");
        assert_eq!(resolve_disk(Some("/abs/d.qcow2"), Some(dir), "g").unwrap(), Path::new("/abs/d.qcow2"));
        assert_eq!(resolve_disk(Some("d.qcow2"), Some(dir), "g").unwrap(), Path::new("/vms/g/d.qcow2"));
        assert_eq!(resolve_disk(None, Some(dir), "g").unwrap(), Path::new("/vms/g/disk.qcow2"));
        assert!(resolve_disk(None, None, "g").is_err());
    }
}
