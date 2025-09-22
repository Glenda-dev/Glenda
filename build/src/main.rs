use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use which::which;

#[derive(Parser, Debug)]
#[command(name = "xtask", version, about = "Glenda Build System")]
struct Xtask {
    #[arg(long)]
    release: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Build the kernel
    Build,
    /// Build then boot the kernel in QEMU
    Run,
    /// Start QEMU paused and wait for GDB
    Gdb,
    /// Disassemble the kernel ELF
    Objdump,
    /// Show section sizes
    Size,
}

fn main() -> anyhow::Result<()> {
    let xtask = Xtask::parse();
    let mode = if xtask.release { "release" } else { "debug" };

    match xtask.cmd {
        Cmd::Build => build(mode)?,
        Cmd::Run => {
            build(mode)?;
            qemu_run(mode)?;
        }
        Cmd::Gdb => {
            build(mode)?;
            qemu_gdb(mode)?;
        }
        Cmd::Objdump => objdump(mode)?,
        Cmd::Size => size(mode)?,
    }
    Ok(())
}

fn elf_path(mode: &str) -> PathBuf {
    Path::new("target")
        .join("riscv64gc-unknown-none-elf")
        .join(mode)
        .join("kernel")
}

fn build(mode: &str) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("-p").arg("kernel").arg("--target").arg("riscv64gc-unknown-none-elf");
    if mode == "release" { cmd.arg("--release"); }
    run(&mut cmd)
}

fn qemu_cmd() -> anyhow::Result<String> {
    let qemu = which("qemu-system-riscv64")
        .map_err(|_| anyhow::anyhow!("[ ERROR ] qemu-system-riscv64 not found in PATH"))?;
    Ok(qemu.to_string_lossy().into_owned())
}

fn qemu_run(mode: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    if !elf.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] ELF not found: {}", elf.display()));
    }
    let qemu = qemu_cmd()?;
    let mut cmd = Command::new(&qemu);
    cmd.args([
        "-machine","virt",
        "-m","128M",
        "-nographic",
        "-bios","default",
        "-kernel", elf.to_str().unwrap(),
    ]);
    run(&mut cmd)
}

fn qemu_gdb(mode: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    if !elf.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] ELF not found: {}", elf.display()));
    }
    let qemu = qemu_cmd()?;
    let mut cmd = Command::new(&qemu);
    cmd.args([
        "-machine","virt",
        "-m","128M",
        "-nographic",
        "-bios","default",
        "-S","-s",
        "-kernel", elf.to_str().unwrap(),
    ]);
    eprintln!("QEMU started. In another shell:");
    if which("gdb").is_ok() {
        eprintln!("  gdb -ex 'set architecture riscv:rv64' -ex 'target remote :1234' -ex 'symbol-file {}'", elf.display());
    } else {
        eprintln!("[ ERROR ] install gdb or riscv64-unknown-elf-gdb first");
    }
    run(&mut cmd)
}

fn objdump(mode: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    let tool = which("riscv64-unknown-elf-objdump")
        .or_else(|_| which("llvm-objdump"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install objdump first"))?;
    let mut cmd = Command::new(tool);
    if cmd.get_program().to_string_lossy().contains("llvm-objdump") {
        cmd.args(["-d","--all-headers","--source", elf.to_str().unwrap()]);
    } else {
        cmd.args(["-d","--all-headers","--source", elf.to_str().unwrap()]);
    }
    run(&mut cmd)
}

fn size(mode: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    let tool = which("riscv64-unknown-elf-size")
        .or_else(|_| which("size"))
        .map_err(|_| anyhow::anyhow!("[ ERROR ] install size first"))?;
    let mut cmd = Command::new(tool);
    cmd.args(["-A", elf.to_str().unwrap()]);
    run(&mut cmd)
}

fn run(cmd: &mut Command) -> anyhow::Result<()> {
    eprintln!("[ INFO ] Running: $ {:?}", cmd);
    let status = cmd.stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("[ ERROR ] command failed with status {}", status));
    }
    Ok(())
}

mod anyhow {
    pub use anyhow::*;
}
use anyhow::*;
