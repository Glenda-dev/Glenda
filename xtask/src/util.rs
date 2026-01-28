use crate::config::Config;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use which::which;

pub fn run(cmd: &mut Command) -> anyhow::Result<()> {
    eprintln!("[ INFO ] Running: $ {:?}", cmd);
    let status =
        cmd.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("[ ERROR ] command failed with status {}", status));
    }
    Ok(())
}

pub fn objdump(cfg: &Config) -> anyhow::Result<()> {
    let elf = PathBuf::from("target")
        .join(cfg.system.arch.target_triple())
        .join(cfg.system.profile.as_str())
        .join("kernel");
    let bin = format!("{}objdump", cfg.system.arch.binutils_prefix());
    let tool = which(&bin).map_err(|_| anyhow::anyhow!("[ ERROR ] install {} first", bin))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
    run(&mut cmd)
}

pub fn size(cfg: &Config) -> anyhow::Result<()> {
    let elf = PathBuf::from("target")
        .join(cfg.system.arch.target_triple())
        .join(cfg.system.profile.as_str())
        .join("kernel");
    let bin = format!("{}size", cfg.system.arch.binutils_prefix());
    let tool = which(&bin).map_err(|_| anyhow::anyhow!("[ ERROR ] install {} first", bin))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-A", elf.to_str().unwrap()]);
    run(&mut cmd)
}
