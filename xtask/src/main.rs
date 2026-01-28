use clap::{Parser, Subcommand};
mod arch;
mod build;
mod config;
mod fs;
mod qemu;
mod util;

use config::Config;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(name = "xtask", version, about = "Glenda Build System")]
struct Xtask {
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
        #[arg(long, default_value_t = 1)]
        cpus: u32,

        /// Memory for QEMU (e.g. 128M, 1G)
        #[arg(long, default_value = "1G")]
        mem: String,

        /// Display device for QEMU. Use "nographic" for serial-only, or a display backend (e.g. "gtk", "sdl", "none").
        #[arg(long, default_value = "nographic")]
        display: String,
    },
    /// Start QEMU paused and wait for GDB
    Gdb {
        /// Number of virtual CPUs to pass to QEMU
        #[arg(long, default_value_t = 1)]
        cpus: u32,

        /// Memory for QEMU (e.g. 128M, 1G)
        #[arg(long, default_value = "1G")]
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
        #[arg(long, default_value = "1G")]
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
    let default_path = "config.toml";
    let cfg_path = Path::new(xtask.config.as_deref().unwrap_or(default_path));
    if !cfg_path.exists() {
        eprintln!("[ WARN ] {} not found, skipping pack step", cfg_path.display());
        return Ok(());
    }
    let cfg = Config::from_path(cfg_path)?;

    match xtask.cmd {
        Cmd::Build => build::build(&cfg)?,
        Cmd::Run { cpus, mem, display } => {
            build::build(&cfg)?;
            fs::mkfs()?;
            qemu::qemu_run(&cfg, cpus, &mem, &display)?;
        }
        Cmd::Gdb { cpus, mem, display, port } => {
            build::build(&cfg)?;
            fs::mkfs()?;
            qemu::qemu_gdb(&cfg, cpus, &mem, &display, port)?;
        }
        Cmd::Objdump => util::objdump(&cfg)?,
        Cmd::Size => util::size(&cfg)?,
        Cmd::Mkfs => fs::mkfs()?,
        Cmd::DumpDtb { cpus, mem } => qemu::qemu_dump_dtb(&cfg, cpus, &mem)?,
        Cmd::Clean => build::clean()?,
    }
    Ok(())
}

mod anyhow {
    pub use anyhow::*;
}
use anyhow::*;
