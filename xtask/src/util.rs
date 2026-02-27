use crate::config::Config;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use which::which;

pub fn run(cmd: &mut Command) -> anyhow::Result<()> {
    run_with_timeout(cmd, None)
}

pub fn run_with_timeout(cmd: &mut Command, timeout: Option<u64>) -> anyhow::Result<()> {
    eprintln!("[ INFO ] Running: $ {:?}", cmd);
    let mut child = cmd
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow::anyhow!("[ ERROR ] Failed to start command {:?}: {}", cmd, e))?;

    if let Some(timeout_secs) = timeout {
        let start = std::time::Instant::now();
        loop {
            match child.try_wait()? {
                Some(status) => {
                    if !status.success() {
                        return Err(anyhow::anyhow!(
                            "[ ERROR ] command failed with status {}: {:?}",
                            status,
                            cmd
                        ));
                    }
                    return Ok(());
                }
                None => {
                    if start.elapsed().as_secs() >= timeout_secs {
                        child.kill()?;
                        return Err(anyhow::anyhow!(
                            "[ ERROR ] command timed out after {} seconds: {:?}",
                            timeout_secs,
                            cmd
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    } else {
        let status = child.wait()?;
        if !status.success() {
            return Err(anyhow::anyhow!(
                "[ ERROR ] command failed with status {}: {:?}",
                status,
                cmd
            ));
        }
        Ok(())
    }
}

pub fn download(url: &str, dest: &Path) -> anyhow::Result<()> {
    if dest.exists() {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let is_bz2 = url.ends_with(".bz2");
    let download_path = if is_bz2 { dest.with_extension("bz2") } else { dest.to_path_buf() };

    eprintln!("[ INFO ] Downloading {} -> {}", url, download_path.display());
    let mut cmd = Command::new("curl");
    cmd.args(["-L", "-o", download_path.to_str().unwrap(), url]);
    run(&mut cmd)?;

    if is_bz2 {
        eprintln!("[ INFO ] Decompressing {}...", download_path.display());
        let mut cmd = Command::new("bunzip2");
        cmd.arg(download_path.to_str().unwrap());
        run(&mut cmd)?;
    }

    Ok(())
}

pub fn objdump(cfg: &Config) -> anyhow::Result<()> {
    let is_uefi = cfg.system.bootloader == crate::arch::Bootloader::Uefi;
    let target = cfg.system.arch.target_triple();
    let kernel_name = if is_uefi { "kernel.efi" } else { "kernel" };
    let elf =
        PathBuf::from("target").join(target).join(cfg.system.profile.as_str()).join(kernel_name);
    let bin = format!("{}objdump", cfg.system.arch.binutils_prefix());
    let tool = which(&bin).map_err(|_| anyhow::anyhow!("[ ERROR ] install {} first", bin))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
    run(&mut cmd)
}

pub fn size(cfg: &Config) -> anyhow::Result<()> {
    let is_uefi = cfg.system.bootloader == crate::arch::Bootloader::Uefi;
    let target = cfg.system.arch.target_triple();
    let kernel_name = if is_uefi { "kernel.efi" } else { "kernel" };
    let elf =
        PathBuf::from("target").join(target).join(cfg.system.profile.as_str()).join(kernel_name);
    let bin = format!("{}size", cfg.system.arch.binutils_prefix());
    let tool = which(&bin).map_err(|_| anyhow::anyhow!("[ ERROR ] install {} first", bin))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-A", elf.to_str().unwrap()]);
    run(&mut cmd)
}

pub fn strip(cfg: &Config, file: &Path) -> anyhow::Result<()> {
    let bin = format!("{}objcopy", cfg.system.arch.binutils_prefix());
    let tool = which(&bin)
        .or_else(|_| which("rust-objcopy"))
        .or_else(|_| which("llvm-objcopy"))
        .or_else(|_| which("objcopy"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install {} or rust-objcopy first", bin))?;
    let mut cmd = Command::new(tool);
    cmd.arg("--strip-all").arg(file);
    run(&mut cmd)
}
