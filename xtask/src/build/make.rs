use crate::config::Config;
use crate::util::run;
use std::path::Path;
use std::process::Command;

pub fn build(cfg: &Config, path: &Path, args: &[String]) -> anyhow::Result<()> {
    let mut cmd = Command::new("make");
    cmd.current_dir(path);
    cmd.env("ARCH", cfg.system.arch.as_str());

    for arg in args {
        cmd.arg(arg);
    }
    if let Ok(n) = std::thread::available_parallelism() {
        cmd.arg(format!("-j{}", n.get()));
    }
    run(&mut cmd)
}
