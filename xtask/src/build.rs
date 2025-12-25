use std::path::PathBuf;
use std::process::Command;

use crate::pack;
use crate::util::run;

pub fn build(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
    // Build libraries
    build_lib(mode, features)?;
    // Process workspace services and generate modules blob for kernel embedding
    pack::process_services()?;
    // Build the kernel
    build_kernel(mode, features)?;
    Ok(())
}

pub fn build_kernel(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("-p").arg("kernel").arg("--target").arg("riscv64gc-unknown-none-elf");
    if mode == "release" {
        cmd.arg("--release");
    }
    if !features.is_empty() {
        let joined = features.join(",");
        cmd.arg("--features").arg(joined);
    }
    run(&mut cmd)
}

pub fn build_lib(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("-p")
        .arg("libglenda-rs")
        .arg("--target")
        .arg("riscv64gc-unknown-none-elf");
    if mode == "release" {
        cmd.arg("--release");
    }
    if !features.is_empty() {
        let joined = features.join(",");
        cmd.arg("--features").arg(joined);
    }
    run(&mut cmd)
}
