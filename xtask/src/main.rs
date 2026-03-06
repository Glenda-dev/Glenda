use clap::{Parser, Subcommand};
mod arch;
mod build;
mod check;
mod config;
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
    /// Build then boot the kernel in QEMU (via ISO image)
    Run {
        #[arg(long, default_value_t = 60)]
        timeout: u64,
    },
    /// Start QEMU paused and wait for GDB
    Gdb {
        #[arg(long, default_value_t = 1234)]
        port: u16,
    },
    /// Disassemble the kernel ELF
    Objdump,
    /// Show section sizes
    Size,
    /// Dump QEMU DTB to target/virt.dtb
    DumpDtb,
    /// Dump QEMU ACPI tables to target/acpi/
    DumpAcpi,
    /// Generate bootable disk image (ISO or FAT32)
    Image {
        #[arg(long)]
        iso: bool,
    },
    Clean,
    /// Run cargo check on all components
    Check {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    // cd into workspace root
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let root = std::path::Path::new(&manifest_dir).parent().unwrap();
    std::env::set_current_dir(root)?;
    let root = std::env::current_dir()?;

    let xtask = Xtask::parse();
    let default_path = "config.toml";
    let cfg_path = root.join(xtask.config.as_deref().unwrap_or(default_path));
    if !cfg_path.exists() {
        eprintln!("[ WARN ] {} not found, skipping pack step", cfg_path.display());
        return Ok(());
    }
    let cfg = Config::from_path(&cfg_path)?;

    match xtask.cmd {
        Cmd::Build => build::build(&cfg)?,
        Cmd::Run { timeout } => {
            if cfg.system.arch == arch::Arch::Hosted {
                run_hosted(&cfg)?;
            } else {
                qemu::qemu_run(&cfg, Some(timeout))?;
            }
        }
        Cmd::Gdb { port } => {
            qemu::qemu_gdb(&cfg, port)?;
        }
        Cmd::Objdump => util::objdump(&cfg)?,
        Cmd::Size => util::size(&cfg)?,
        Cmd::DumpDtb => qemu::qemu_dump_dtb(&cfg)?,
        Cmd::DumpAcpi => qemu::qemu_dump_acpi(&cfg)?,
        Cmd::Image { iso } => {
            if iso {
                build::image_iso(&cfg)?;
            } else {
                build::image_img(&cfg)?;
            }
        }
        Cmd::Clean => build::clean(&cfg)?,
        Cmd::Check { args } => check::check(&cfg, &args)?,
    }
    Ok(())
}

fn run_hosted(cfg: &Config) -> anyhow::Result<()> {
    use std::process::Command;
    let runtime_path = &cfg.hosted.runtime;
    let profile = &cfg.system.profile;
    // Assuming the binary name matches the folder name
    let runtime_bin = Path::new(runtime_path).file_name().unwrap().to_str().unwrap();
    let runtime_exe = Path::new("target").join(profile).join(runtime_bin);
    let socket = &cfg.hosted.socket;

    eprintln!("[ INFO ] Running Hosted Runtime: {}", runtime_exe.display());

    let mut cmd = Command::new(runtime_exe);
    cmd.arg("--listen").arg(socket);
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Runtime exited with error");
    }
    Ok(())
}

mod anyhow {
    pub use anyhow::*;
}
use anyhow::*;
