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

        /// Run tests instead of normal kernel
        #[arg(long, default_value_t = false)]
        test: bool,
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
        Cmd::Gdb { cpus, mem, display, test } => {
            let mut feats = xtask.features.clone();
            if test == true {
                if !feats.iter().any(|f| f == "tests") {
                    feats.push(String::from("tests"));
                }
            }
            build(mode, &feats)?;
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
    build_service(mode, features)?;
    link_service(mode, features)?;
    build_kernel(mode, features)?;
    Ok(())
}

fn build_kernel(mode: &str, features: &Vec<String>) -> anyhow::Result<()> {
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

fn build_service(mode: &str, _features: &Vec<String>) -> anyhow::Result<()> {
    let mut cmd = Command::new("make");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    std::fs::create_dir_all(format!("{}/../target/service/hello", manifest_dir)).unwrap();
    cmd.current_dir(format!("{}/../service/hello", manifest_dir));
    cmd.arg("CROSS_COMPILE=riscv64-unknown-elf-");
    run(&mut cmd)
}

// Mode and Features are expected to be unused now
fn link_service(_mode: &str, _features: &Vec<String>) -> anyhow::Result<()> {
    let service_bin =
        std::path::Path::new("target").join("service").join("hello").join("hello.bin");
    let service_bin_str = "service/hello/hello.bin";
    let out_file = std::path::Path::new("target").join("proc_payload.rs");
    if service_bin.exists() {
        let content = format!(
            "pub const PROC_PAYLOAD: &[u8] = include_bytes!(\"{}\");\npub const HAS_PROC_PAYLOAD: bool = true;\n",
            service_bin_str
        );
        std::fs::write(&out_file, content).unwrap();
    } else {
        println!(
            "[ WARN ] Service binary not found: {}, generating empty payload",
            service_bin.display(),
        );
        let content = String::from(
            "pub const PROC_PAYLOAD: &[u8] = &[];\npub const HAS_PROC_PAYLOAD: bool = false;\n",
        );
        std::fs::write(&out_file, content).unwrap();
    }
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
