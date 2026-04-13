use crate::config::Config;
use privilege::runas::Command as PrivilegedCommand;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub fn mount_rootfs(cfg: &Config, workspace_root: &Path) -> anyhow::Result<()> {
    let image = resolve_rootfs_image(cfg, workspace_root)?;

    let mountpoint = workspace_root.join("mnt");
    fs::create_dir_all(&mountpoint)?;
    let mountpoint = fs::canonicalize(&mountpoint)?;

    if is_mounted(&mountpoint)? {
        eprintln!("[ INFO ] {} is already mounted at {}", image.display(), mountpoint.display());
        return Ok(());
    }

    eprintln!("[ INFO ] Mounting rootfs image {} -> {}", image.display(), mountpoint.display());

    let args = vec![
        OsString::from("-o"),
        OsString::from("loop"),
        image.as_os_str().to_os_string(),
        mountpoint.as_os_str().to_os_string(),
    ];
    let status = run_privileged_command("mount", &args)?;

    if !status.success() {
        anyhow::bail!(
            "[ ERROR ] mount failed (status: {}). You may need root privileges (e.g. run with sudo).",
            status
        );
    }

    eprintln!("[ INFO ] Mounted at {}", mountpoint.display());
    Ok(())
}

pub fn umount_rootfs(workspace_root: &Path) -> anyhow::Result<()> {
    let mountpoint = workspace_root.join("mnt");
    if !mountpoint.exists() {
        eprintln!("[ INFO ] {} does not exist, nothing to unmount", mountpoint.display());
        return Ok(());
    }

    let mountpoint = fs::canonicalize(&mountpoint)?;
    if !is_mounted(&mountpoint)? {
        eprintln!("[ INFO ] {} is not mounted", mountpoint.display());
        return Ok(());
    }

    eprintln!("[ INFO ] Unmounting {}", mountpoint.display());
    let args = vec![mountpoint.as_os_str().to_os_string()];
    let status = run_privileged_command("umount", &args)?;

    if !status.success() {
        anyhow::bail!(
            "[ ERROR ] umount failed (status: {}). You may need root privileges (e.g. run with sudo).",
            status
        );
    }

    eprintln!("[ INFO ] Unmounted {}", mountpoint.display());
    Ok(())
}

fn resolve_rootfs_image(cfg: &Config, workspace_root: &Path) -> anyhow::Result<PathBuf> {
    let configured = cfg.qemu.disk.as_deref().map(str::trim).filter(|s| !s.is_empty());

    if let Some(path) = configured {
        let p = Path::new(path);
        let resolved = if p.is_absolute() { p.to_path_buf() } else { workspace_root.join(p) };
        if resolved.exists() {
            return Ok(resolved);
        }
        anyhow::bail!("[ ERROR ] configured rootfs image not found: {}", resolved.display());
    }

    anyhow::bail!("[ ERROR ] no rootfs image configured. Please set [qemu].disk or [qemu].drive.")
}

fn is_mounted(mountpoint: &Path) -> anyhow::Result<bool> {
    let mounts = fs::read_to_string("/proc/self/mounts")?;
    let target = mountpoint.to_string_lossy();

    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let _src = parts.next();
        let dst = parts.next();

        if let Some(dst) = dst {
            if unescape_mount_path(dst) == target {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn unescape_mount_path(s: &str) -> String {
    s.replace("\\040", " ").replace("\\011", "\t").replace("\\012", "\n").replace("\\134", "\\")
}

fn run_privileged_command(program: &str, args: &[OsString]) -> anyhow::Result<ExitStatus> {
    if privilege::user::privileged() {
        return Command::new(program).args(args).status().map_err(|e| {
            anyhow::anyhow!("[ ERROR ] failed to execute {} command: {}", program, e)
        });
    }

    let mut cmd = PrivilegedCommand::new(program);
    cmd.force_prompt(false);
    for arg in args {
        cmd.arg(arg);
    }

    cmd.run().map_err(|e| {
        anyhow::anyhow!("[ ERROR ] failed to run {} with elevated privileges: {}", program, e)
    })
}
