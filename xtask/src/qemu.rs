use crate::util::run;
use std::path::PathBuf;
use std::process::Command;
use crate::config::Config;
use which::which;

pub fn qemu_cmd(cfg: &Config) -> anyhow::Result<String> {
    let qemu_arch = cfg.system.arch.qemu_binary();
    let qemu = which(qemu_arch)
        .map_err(|_| anyhow::anyhow!("[ ERROR ] {} not found in PATH", qemu_arch))?;
    Ok(qemu.to_string_lossy().into_owned())
}

pub fn qemu_run(cfg: &Config, cpus: u32, mem: &str, display: &str) -> anyhow::Result<()> {
    let elf = PathBuf::from("target")
        .join(cfg.system.arch.target_triple())
        .join(cfg.system.profile.as_str())
        .join("kernel");
    if !elf.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] ELF not found: {}", elf.display()));
    }
    let qemu = qemu_cmd(cfg)?;
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
    cmd.arg("-drive").arg("file=disk.img,if=none,format=raw,id=x0");
    cmd.arg("-device").arg("virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0");
    cmd.arg("-initrd").arg("target/modules.bin");
    cmd.arg("-append").arg("console=ttyS0 loglevel=7");
    cmd.arg("-bios").arg("default").arg("-kernel").arg(elf.to_str().unwrap());
    run(&mut cmd)
}

pub fn qemu_gdb(
    cfg: &Config,
    cpus: u32,
    mem: &str,
    display: &str,
    port: u16,
) -> anyhow::Result<()> {
    let elf = PathBuf::from("target").join(cfg.system.arch.target_triple()).join(cfg.system.profile.as_str()).join("kernel");
    if !elf.exists() {
        return Err(anyhow::anyhow!("[ ERROR ] ELF not found: {}", elf.display()));
    }
    let qemu = qemu_cmd(cfg)?;
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
    cmd.arg("-drive").arg("file=disk.img,if=none,format=raw,id=x0");
    cmd.arg("-device").arg("virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0");
    cmd.arg("-initrd").arg("target/modules.bin");
    cmd.arg("-append").arg("console=ttyS0 loglevel=7");
    cmd.arg("-gdb").arg(format!("tcp::{port}"));
    cmd.arg("-bios").arg("default").arg("-S").arg("-kernel").arg(elf.to_str().unwrap());
    eprintln!("QEMU started. In another shell:");
    if which("gdb").is_ok() {
        eprintln!(
            "  gdb -ex 'set architecture riscv:rv64' -ex 'target remote :{}' -ex 'symbol-file {}'",
            port,
            elf.display()
        );
    } else {
        eprintln!("[ ERROR ] install gdb or riscv64-elf-gdb first");
    }
    run(&mut cmd)
}

pub fn qemu_dump_dtb(cfg: &Config, cpus: u32, mem: &str) -> anyhow::Result<()> {
    let qemu = qemu_cmd(cfg)?;
    let mut cmd = Command::new(&qemu);
    let dtb_path = "target/virt.dtb";
    cmd.arg("-machine").arg(format!("virt,dumpdtb={}", dtb_path));
    // CPUs
    if cpus > 1 {
        cmd.arg("-smp").arg(cpus.to_string());
    }
    // Memory
    cmd.arg("-m").arg(mem);
    cmd.arg("-display").arg("none");

    eprintln!("[ INFO ] Dumping DTB to {}...", dtb_path);
    run(&mut cmd)?;
    eprintln!("[ INFO ] DTB dumped successfully.");
    eprintln!(
        "[ INFO ] You can decompile it with: dtc -I dtb -O dts -o target/virt.dts {}",
        dtb_path
    );
    Ok(())
}
