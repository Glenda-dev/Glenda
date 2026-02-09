use crate::config::Config;
use crate::util::run;
use std::path::Path;
use std::process::Command;

pub fn check(cfg: &Config, args: &[String]) -> anyhow::Result<()> {
    // 1. Check Kernel
    check_kernel(cfg, args)?;

    // 2. Check Libraries
    for lib in &cfg.libraries {
        if lib.build == "cargo" {
            eprintln!("[ INFO ] Checking Library {}", lib.name);
            check_crate(cfg, Path::new(&lib.path), &lib.name, args)?;
        }
    }

    // 3. Check Services
    for svc in &cfg.services {
        if svc.build == "cargo" {
            eprintln!("[ INFO ] Checking Service {}", svc.name);
            check_crate(cfg, Path::new(&svc.path), &svc.name, args)?;
        }
    }

    Ok(())
}

fn check_kernel(cfg: &Config, args: &[String]) -> anyhow::Result<()> {
    eprintln!("[ INFO ] Checking Kernel");
    let features = cfg.features.get("kernel").map(|arr| arr.join(",")).unwrap_or_default();
    let mut cmd = Command::new("cargo");
    cmd.current_dir("kernel");
    cmd.arg("check");
    // Pass external args first or last? rust-analyzer usually appends.
    // If args contains "--", cargo might get confused if we mix.
    // But typically `cargo check [args]` works.
    cmd.args(args);

    cmd.arg("--target").arg(cfg.system.arch.target_triple());
    cmd.arg("--profile").arg(&cfg.system.profile);
    if !features.is_empty() {
        cmd.arg("--features").arg(features);
    }

    run(&mut cmd)?;
    Ok(())
}

fn check_crate(cfg: &Config, path: &Path, name: &str, args: &[String]) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(path);
    cmd.arg("check");
    cmd.args(args);
    cmd.arg("--target").arg(cfg.system.arch.target_triple());
    cmd.arg("--profile").arg(&cfg.system.profile);

    let features = cfg.features.get(name).map(|arr| arr.join(",")).unwrap_or_default();
    if !features.is_empty() {
        cmd.arg("--features").arg(features);
    }

    // Libraries and services do not need linker script injection usually

    run(&mut cmd)?;
    Ok(())
}
