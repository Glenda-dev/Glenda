use crate::config::Config;
use crate::util::run;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn build(cfg: &Config, path: &Path, args: &[String]) -> anyhow::Result<()> {
    let build_dir = path.join("build");
    fs::create_dir_all(&build_dir)?;

    let mut cmd = Command::new("cmake");
    cmd.current_dir(&build_dir);
    cmd.arg("..");
    cmd.arg(format!("-DARCH={}", cfg.system.arch.as_str()));

    // Set cross-compiler if generic (bare metal)
    // We assume the prefix matches the GCC toolchain
    let cc = format!("{}gcc", cfg.system.arch.binutils_prefix());
    cmd.arg(format!("-DCMAKE_C_COMPILER={}", cc));
    cmd.arg(format!("-DCMAKE_ASM_COMPILER={}", cc));
    cmd.arg("-DCMAKE_SYSTEM_NAME=Generic");
    cmd.arg("-DCMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY");
    // Force compiler check to pass for bare metal / kernel lib without stdlib
    cmd.arg("-DCMAKE_C_COMPILER_WORKS=1");
    cmd.arg("-DCMAKE_ASM_COMPILER_WORKS=1");
    // Install locally within the build directory to avoid permission issues
    // and for easier packaging later
    let cwd = std::env::current_dir()?;
    let install_prefix = cwd.join("target/lib");
    cmd.arg(format!("-DCMAKE_INSTALL_PREFIX={}", install_prefix.display()));

    for arg in args {
        cmd.arg(arg);
    }
    run(&mut cmd)?;

    let mut cmd = Command::new("make");
    cmd.current_dir(&build_dir);
    if let Ok(n) = std::thread::available_parallelism() {
        cmd.arg(format!("-j{}", n.get()));
    }
    run(&mut cmd)?;
    let mut cmd = Command::new("make");
    cmd.current_dir(&build_dir);
    cmd.arg("install");
    run(&mut cmd)?;

    Ok(())
}
