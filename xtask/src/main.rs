//! Host-side build/run driver for the Glenda kernel workspace.
//!
//! Commands:
//!   cargo xtask build            Build the kernel for riscv64gc-unknown-none-elf.
//!   cargo xtask run  [--mem M]   Build then boot the kernel under QEMU virt.
//!
//! Everything shells out to `cargo`/`qemu-system-riscv64` and uses only the
//! standard library so the driver itself never needs crates.io (offline-safe).

use std::env;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const TARGET: &str = "riscv64gc-unknown-none-elf";
const KERNEL_ELF: &str = "target/riscv64gc-unknown-none-elf/debug/kernel";

fn workspace_root() -> PathBuf {
    // xtask runs with cwd = workspace root (VOS runners set cwd: ".").
    let cwd = env::current_dir().expect("failed to read cwd");
    if cwd.join("xtask").is_dir() && cwd.join("kernel").is_dir() {
        return cwd;
    }
    // Fallback: we are inside xtask/
    let parent = cwd
        .parent()
        .expect("xtask has no parent directory")
        .to_path_buf();
    if parent.join("xtask").is_dir() && parent.join("kernel").is_dir() {
        return parent;
    }
    panic!(
        "xtask must run from the workspace root or from xtask/ (cwd={})",
        cwd.display()
    );
}

fn run(cmd: &mut Command) -> i32 {
    let status = cmd.status().unwrap_or_else(|e| {
        eprintln!("xtask: failed to run {:?}: {}", cmd.get_program(), e);
        process::exit(2);
    });
    status.code().unwrap_or(1)
}

fn cargo_build_kernel(root: &Path) -> i32 {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--target")
        .arg(TARGET)
        .arg("-p")
        .arg("kernel")
        .current_dir(root);
    run(&mut cmd)
}

fn ships(desc: &str) -> i32 {
    eprintln!("xtask: unknown command '{desc}'");
    eprintln!("usage: cargo xtask build | run [--mem M]");
    2
}

fn main() {
    let mut args = env::args().skip(1);
    let sub = args.next().unwrap_or_default();
    let root = workspace_root();

    match sub.as_str() {
        "build" => {
            let code = cargo_build_kernel(&root);
            if code != 0 {
                process::exit(code);
            }
            let elf = root.join(KERNEL_ELF);
            if !elf.is_file() {
                eprintln!("xtask: expected kernel ELF at {} (missing)", elf.display());
                process::exit(1);
            }
            eprintln!("xtask: kernel built -> {}", elf.display());
        }
        "run" => {
            let mut mem = "128M".to_string();
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--mem" => {
                        if let Some(m) = args.next() {
                            mem = m;
                        }
                    }
                    other => {
                        let code = ships(other);
                        process::exit(code);
                    }
                }
            }
            let code = cargo_build_kernel(&root);
            if code != 0 {
                process::exit(code);
            }
            let elf = root.join(KERNEL_ELF);
            if !elf.is_file() {
                eprintln!("xtask: kernel ELF missing at {}", elf.display());
                process::exit(1);
            }
            // Boot under QEMU virt with OpenSBI firmware; serial goes to stdio
            // so the GLENDA_BOOT_OK banner is captured on stdout.
            let mut qemu = Command::new("qemu-system-riscv64");
            qemu.arg("-machine")
                .arg("virt")
                .arg("-m")
                .arg(&mem)
                .arg("-bios")
                .arg("default")
                .arg("-kernel")
                .arg(&elf)
                .arg("-nographic")
                .arg("-serial")
                .arg("mon:stdio")
                .arg("-no-reboot")
                .current_dir(&root);
            process::exit(run(&mut qemu));
        }
        other => {
            let code = ships(other);
            process::exit(code);
        }
    }
}