use crate::config::Config;
use crate::util::run;
use std::path::Path;
use std::process::Command;

/// Run cargo build for a component
pub fn build(cfg: &Config, path: &Path, features: &str, flags: &str) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(path);
    cmd.arg("build");
    let target = cfg.system.arch.target_triple();
    cmd.arg("--target").arg(target);
    cmd.arg("--profile").arg(&cfg.system.profile);
    if !features.is_empty() {
        cmd.arg("--features").arg(features);
    }
    if !flags.is_empty() {
        cmd.env("RUSTFLAGS", flags);
    }
    run(&mut cmd)
}
