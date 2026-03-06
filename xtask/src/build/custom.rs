use crate::config::Config;
use crate::util::run;
use std::path::Path;
use std::process::Command;

pub fn build(cfg: &Config, path: &Path, args: &[String]) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let path = root.join(path);
    let build_script = path.join("build.sh");
    if !build_script.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] build.sh not found in {}", path.display()));
    }

    eprintln!("[ INFO ] Running custom build script in {}", path.display());
    let mut cmd = Command::new("sh");
    cmd.current_dir(&path);
    cmd.arg("./build.sh");
    cmd.arg(format!("--arch={}", cfg.system.arch.as_str()));
    for arg in args {
        cmd.arg(arg);
    }
    cmd.stdout(std::process::Stdio::null());
    run(&mut cmd)
}
