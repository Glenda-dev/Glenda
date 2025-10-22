use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use which::which;

#[derive(Parser, Debug)]
#[command(name = "xtask", version, about = "Glenda Build System")]
struct Xtask {
    #[arg(long, global = true)]
    release: bool,

    #[arg(long = "features", value_delimiter = ',', num_args(0..), global = true)]
    features: Vec<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Build the kernel
    Build,
    /// Build then boot the kernel in QEMU
    Run {
        /// Number of virtual CPUs to pass to QEMU
        #[arg(long, default_value_t = 4)]
        cpus: u32,

        /// Memory for QEMU (e.g. 128M, 1G)
        #[arg(long, default_value = "128M")]
        mem: String,

        /// Display device for QEMU. Use "nographic" for serial-only, or a display backend (e.g. "gtk", "sdl", "none").
        #[arg(long, default_value = "nographic")]
        display: String,
    },
    /// Run kernel tests
    Test {
        /// Number of virtual CPUs to pass to QEMU
        #[arg(long, default_value_t = 4)]
        cpus: u32,

        /// Memory for QEMU (e.g. 128M, 1G)
        #[arg(long, default_value = "128M")]
        mem: String,

        /// Display device for QEMU. Use "nographic" for serial-only, or a display backend (e.g. "gtk", "sdl", "none").
        #[arg(long, default_value = "nographic")]
        display: String,
    },
    /// Start QEMU paused and wait for GDB
    Gdb {
        /// Number of virtual CPUs to pass to QEMU
        #[arg(long, default_value_t = 4)]
        cpus: u32,

        /// Memory for QEMU (e.g. 128M, 1G)
        #[arg(long, default_value = "128M")]
        mem: String,

        /// Display device for QEMU. Use "nographic" for serial-only, or a display backend (e.g. "gtk", "sdl", "none").
        #[arg(long, default_value = "nographic")]
        display: String,
    },
    /// Disassemble the kernel ELF
    Objdump,
    /// Show section sizes
    Size,
}

fn main() -> anyhow::Result<()> {
    let xtask = Xtask::parse();
    let mode = if xtask.release { "release" } else { "debug" };

    match xtask.cmd {
        Cmd::Build => build(mode, &xtask.features)?,
        Cmd::Run { cpus, mem, display } => {
            build(mode, &xtask.features)?;
            qemu_run(mode, cpus, &mem, &display)?;
        }
        Cmd::Gdb { cpus, mem, display } => {
            build(mode, &xtask.features)?;
            qemu_gdb(mode, cpus, &mem, &display)?;
        }
        Cmd::Test { cpus, mem, display } => {
            let mut feats = xtask.features.clone();
            if !feats.iter().any(|f| f == "tests") {
                feats.push(String::from("tests"));
            }
            build(mode, &feats)?;
            qemu_run(mode, cpus, &mem, &display)?;
        }
        Cmd::Objdump => objdump(mode)?,
        Cmd::Size => size(mode)?,
    }
    Ok(())
}

fn elf_path(mode: &str) -> PathBuf {
    Path::new("target").join("riscv64gc-unknown-none-elf").join(mode).join("kernel")
}

fn build(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
    build_service_bin()?;
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

fn build_service_bin() -> anyhow::Result<()> {
    use std::fs;
    let hello_c = Path::new("service").join("hello.c");
    if !hello_c.exists() {
        return Ok(());
    }
    let start_s = Path::new("service").join("start.S");
    let out_elf = Path::new("service").join("hello.elf");
    let out_bin = Path::new("service").join("hello.bin");
    let gcc = which("riscv64-unknown-elf-gcc").or_else(|_| which("riscv64-linux-gnu-gcc"));
    let objcopy = which("riscv64-unknown-elf-objcopy").or_else(|_| which("llvm-objcopy"));
    if gcc.is_err() || objcopy.is_err() {
        eprintln!("[ WARN ] RISC-V gcc/objcopy not found; skipping service/hello build");
        return Ok(());
    }
    let gcc = gcc?;
    let objcopy = objcopy?;
    // Compile ELF
    let mut cmd = Command::new(&gcc);
    cmd.args([
        "-nostdlib",
        "-ffreestanding",
        "-fno-builtin",
        "-fno-stack-protector",
        "-march=rv64gc",
        "-mabi=lp64d",
        "-Os",
        "-Wl,-n",
        "-Wl,--build-id=none",
        "-Wl,-Ttext=0",
        "-I",
        "include",
        start_s.to_str().unwrap(),
        hello_c.to_str().unwrap(),
        "-o",
        out_elf.to_str().unwrap(),
    ]);
    run(&mut cmd)?;
    let mut oc = Command::new(&objcopy);
    if oc.get_program().to_string_lossy().contains("llvm-objcopy") {
        oc.args(["-O", "binary", out_elf.to_str().unwrap(), out_bin.to_str().unwrap()]);
    } else {
        oc.args(["-O", "binary", out_elf.to_str().unwrap(), out_bin.to_str().unwrap()]);
    }
    run(&mut oc)?;
    eprintln!("[ INFO ] Built service/hello.bin for embedding");
    let _ = fs::metadata(&out_bin)?;
    Ok(())
}

fn qemu_cmd() -> anyhow::Result<String> {
    let qemu = which("qemu-system-riscv64")
        .map_err(|_| anyhow::anyhow!("[ ERROR ] qemu-system-riscv64 not found in PATH"))?;
    Ok(qemu.to_string_lossy().into_owned())
}

fn qemu_run(mode: &str, cpus: u32, mem: &str, display: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    if !elf.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] ELF not found: {}", elf.display()));
    }
    let qemu = qemu_cmd()?;
    let mut cmd = Command::new(&qemu);
    cmd.arg("-machine").arg("virt");
    // CPUs
    if cpus > 1 {
        cmd.arg("-smp").arg(cpus.to_string());
    }
    // Memory
    cmd.arg("-m").arg(mem);
    // Display handling: keep legacy -nographic behavior when requested
    if display == "nographic" {
        cmd.arg("-nographic");
    } else if display == "none" {
        cmd.arg("-display").arg("none");
    } else {
        // pass raw display backend name (e.g. gtk, sdl)
        cmd.arg("-display").arg(display);
    }
    cmd.arg("-bios").arg("default").arg("-kernel").arg(elf.to_str().unwrap());
    run(&mut cmd)
}

fn qemu_gdb(mode: &str, cpus: u32, mem: &str, display: &str) -> anyhow::Result<()> {
    let elf = elf_path(mode);
    if !elf.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] ELF not found: {}", elf.display()));
    }
    let qemu = qemu_cmd()?;
    let mut cmd = Command::new(&qemu);
    cmd.arg("-machine").arg("virt");
    // CPUs
    if cpus > 1 {
        cmd.arg("-smp").arg(cpus.to_string());
    }
    // Memory
    cmd.arg("-m").arg(mem);
    // Display handling
    if display == "nographic" {
        cmd.arg("-nographic");
    } else if display == "none" {
        cmd.arg("-display").arg("none");
    } else {
        cmd.arg("-display").arg(display);
    }
    cmd.arg("-bios").arg("default").arg("-S").arg("-s").arg("-kernel").arg(elf.to_str().unwrap());
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
        cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
    } else {
        cmd.args(["-d", "--all-headers", "--source", elf.to_str().unwrap()]);
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
    let status =
        cmd.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("[ ERROR ] command failed with status {}", status));
    }
    Ok(())
}

mod anyhow {
    pub use anyhow::*;
}
use anyhow::*;
