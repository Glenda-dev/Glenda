use crate::config::Config;
use crate::util::run;
use std::path::Path;
use std::process::Command;

pub fn build(cfg: &Config, path: &Path, args: &[String]) -> anyhow::Result<()> {
    // 1. Configure step (if exists)
    let configure = path.join("configure");
    if configure.exists() {
        eprintln!("[ INFO ] Configuring in {}", path.display());
        let mut cmd = Command::new("./configure");
        cmd.current_dir(path);
        // Pass generic env vars
        cmd.env("ARCH", cfg.system.arch.as_str());
        cmd.env("CROSS_COMPILE", cfg.system.arch.binutils_prefix());

        // Pass features as args to configure
        for arg in args {
            cmd.arg(arg);
        }
        run(&mut cmd)?;
    }

    // 2. Make step
    let mut cmd = Command::new("make");
    cmd.current_dir(path);
    cmd.env("ARCH", cfg.system.arch.as_str());
    cmd.env("CROSS_COMPILE", cfg.system.arch.binutils_prefix());

    // Only pass args to make if we DID NOT run configure
    if !configure.exists() {
        for arg in args {
            cmd.arg(arg);
        }
    }

    if let Ok(n) = std::thread::available_parallelism() {
        cmd.arg(format!("-j{}", n.get()));
    }
    run(&mut cmd)
}
