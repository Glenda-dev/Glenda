use clap::{Parser, Subcommand};
mod build;
mod config;
mod fs;
mod qemu;
mod util;

#[derive(Parser, Debug)]
#[command(name = "xtask", version, about = "Glenda Build System")]
struct Xtask {
    #[arg(long, global = true)]
    release: bool,

    #[arg(short, long, global = true)]
    config: Option<String>,

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

        #[arg(long, default_value_t = 1234)]
        port: u16,
    },
    /// Disassemble the kernel ELF
    Objdump,
    /// Show section sizes
    Size,
    /// Generate disk.img
    Mkfs,
    /// Dump QEMU DTB to target/virt.dtb
    DumpDtb {
        /// Number of virtual CPUs
        #[arg(long, default_value_t = 4)]
        cpus: u32,

        /// Memory for QEMU
        #[arg(long, default_value = "128M")]
        mem: String,
    },
    Clean,
}

fn main() -> anyhow::Result<()> {
    // cd into workspace root
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let root = std::path::Path::new(&manifest_dir).parent().unwrap();
    std::env::set_current_dir(root)?;

    let xtask = Xtask::parse();
    let mode = if xtask.release { "release" } else { "debug" };

    match xtask.cmd {
        Cmd::Build => build::build(mode, xtask.config.as_deref())?,
        Cmd::Run { cpus, mem, display } => {
            build::build(mode, xtask.config.as_deref())?;
            fs::mkfs()?;
            qemu::qemu_run(mode, cpus, &mem, &display)?;
        }
        Cmd::Gdb { cpus, mem, display, port } => {
            build::build(mode, xtask.config.as_deref())?;
            fs::mkfs()?;
            qemu::qemu_gdb(mode, cpus, &mem, &display, port)?;
        }
        Cmd::Test { cpus, mem, display } => {
            build::build(mode, xtask.config.as_deref())?;
            fs::mkfs()?;
            qemu::qemu_run(mode, cpus, &mem, &display)?;
        }
        Cmd::Objdump => util::objdump(mode)?,
        Cmd::Size => util::size(mode)?,
        Cmd::Mkfs => fs::mkfs()?,
        Cmd::DumpDtb { cpus, mem } => qemu::qemu_dump_dtb(cpus, &mem)?,
        Cmd::Clean => build::clean()?,
    }
    Ok(())
}

mod anyhow {
    pub use anyhow::*;
}
use anyhow::*;
