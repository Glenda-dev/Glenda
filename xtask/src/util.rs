use std::process::{Command, Stdio};
use which::which;
use std::path::PathBuf;

pub fn run(cmd: &mut Command) -> anyhow::Result<()> {
    eprintln!("[ INFO ] Running: $ {:?}", cmd);
    let status =
        cmd.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("[ ERROR ] command failed with status {}", status));
    }
    Ok(())
}

pub fn objdump(mode: &str) -> anyhow::Result<()> {
    let elf = PathBuf::from("target").join("riscv64gc-unknown-none-elf").join(mode).join("kernel");
    let tool = which("riscv64-elf-objdump")
        .or_else(|_| which("llvm-objdump"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install objdump first"))?;
    let mut cmd = Command::new(tool);
    if cmd.get_program().to_string_lossy().contains("llvm-objdump") {
        cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
    } else {
        cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
    }
    run(&mut cmd)
}

pub fn size(mode: &str) -> anyhow::Result<()> {
    let elf = PathBuf::from("target").join("riscv64gc-unknown-none-elf").join(mode).join("kernel");
    let tool = which("riscv64-elf-size")
        .or_else(|_| which("size"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install size first"))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-A", elf.to_str().unwrap()]);
    run(&mut cmd)
}
