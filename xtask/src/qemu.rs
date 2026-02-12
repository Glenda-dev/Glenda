use crate::config::Config;
use crate::util::run;
use std::path::PathBuf;
use std::process::Command;
use which::which;

pub fn qemu_cmd(cfg: &Config) -> anyhow::Result<String> {
    let qemu_arch = cfg.system.arch.qemu_binary();
    let qemu = which(qemu_arch)
        .map_err(|_| anyhow::anyhow!("[ ERROR ] {} not found in PATH", qemu_arch))?;
    Ok(qemu.to_string_lossy().into_owned())
}

pub fn qemu_run(cfg: &Config) -> anyhow::Result<()> {
    // Check for Disk image instead of bare kernel ELFs for direct bool behavior
    let img = PathBuf::from("target/disk.img");
    if !img.exists() {
        return Err(anyhow::anyhow!(
            "[ ERROR ] target/disk.img not found. Run `cargo xtask image` first."
        ));
    }

    let qemu = qemu_cmd(cfg)?;
    let mut cmd = Command::new(&qemu);
    cmd.arg("-machine").arg("virt");

    // BIOS/Firmware handling (OVMF/EDK2)
    if let Some(bios) = &cfg.qemu.bios {
        // User provided specific BIOS path
        if PathBuf::from(bios).exists() {
            cmd.arg("-drive").arg(format!("if=pflash,format=raw,unit=0,file={},readonly=on", bios));
        } else {
            // Maybe it's just a keyword or QEMU resource name
            cmd.arg("-bios").arg(bios);
        }
    } else {
        let candidates = cfg.system.arch.uefi_firmware_candidates();

        if let Some(path) = candidates.iter().find(|p| PathBuf::from(p).exists()) {
            eprintln!("[ INFO ] Using UEFI Firmware: {}", path);
            cmd.arg("-drive").arg(format!("if=pflash,format=raw,unit=0,file={},readonly=on", path));
        } else {
            return Err(anyhow::anyhow!(
                "[ ERROR ] No UEFI firmware found for {}. Please install edk2/ovmf or specify `bios` in config.toml.",
                cfg.system.arch.as_str()
             ));
        }
    }

    // CPUs
    if cfg.qemu.cpus > 1 {
        cmd.arg("-smp").arg(cfg.qemu.cpus.to_string());
    }
    // Memory
    cmd.arg("-m").arg(&cfg.qemu.mem);
    // Display handling
    if let Some(display) = &cfg.qemu.display.as_str().split(',').next() {
        if *display == "nographic" {
            cmd.arg("-nographic");
        } else if *display == "none" {
            cmd.arg("-display").arg("none");
        } else {
            // Add GPU device for graphical output
            cmd.arg("-device").arg("virtio-gpu-device");
            // Add input devices
            cmd.arg("-device").arg("virtio-keyboard-pci");
            cmd.arg("-device").arg("virtio-mouse-pci");
            // cmd.arg("-device").arg("virtio-tablet-pci");

            cmd.arg("-display").arg(display);

            // Add serial to stdio even if graphical
            cmd.arg("-serial").arg("stdio");
        }
    }
    if let Some(drive) = &cfg.qemu.drive {
        cmd.arg("-drive").arg(format!("file={drive},if=none,format=raw,id=x0"));
        cmd.arg("-device").arg("virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0");
    }

    if cfg.qemu.net {
        let net_type = cfg.qemu.net_type.as_deref().unwrap_or("user");
        let mut netdev = format!("{},id=net0", net_type);
        if net_type == "user" {
            if let Some(hostfwds) = &cfg.qemu.hostfwd {
                for fwd in hostfwds {
                    netdev.push_str(&format!(",hostfwd={}", fwd));
                }
            }
        }
        cmd.arg("-netdev").arg(netdev);

        let mut device = String::from("virtio-net-device,netdev=net0,bus=virtio-mmio-bus.1");
        if let Some(mac) = &cfg.qemu.mac {
            device.push_str(&format!(",mac={}", mac));
        }
        cmd.arg("-device").arg(device);
    }

    // Boot from Disk Image
    // For UEFI boot, a simple raw drive usually works
    cmd.arg("-drive").arg(format!("file={},format=raw", img.display()));

    // Pass bootargs via QEMU is tricky with ISO boot unless editing config.
    // So we ignore cfg.qemu.bootargs here as it's baked into limine.conf by xtask image.

    run(&mut cmd)
}

pub fn qemu_gdb(cfg: &Config, port: u16) -> anyhow::Result<()> {
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
    if cfg.qemu.cpus > 1 {
        cmd.arg("-smp").arg(cfg.qemu.cpus.to_string());
    }
    // Memory
    cmd.arg("-m").arg(&cfg.qemu.mem);
    // Display handling
    let display = &cfg.qemu.display;
    if display == "nographic" {
        cmd.arg("-nographic");
    } else if display == "none" {
        cmd.arg("-display").arg("none");
    } else {
        cmd.arg("-display").arg(display);
    }
    if let Some(drive) = &cfg.qemu.drive {
        cmd.arg("-drive").arg(format!("file={drive},if=none,format=raw,id=x0"));
        cmd.arg("-device").arg("virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0");
    }

    if cfg.qemu.net {
        let net_type = cfg.qemu.net_type.as_deref().unwrap_or("user");
        let mut netdev = format!("{},id=net0", net_type);
        if net_type == "user" {
            if let Some(hostfwds) = &cfg.qemu.hostfwd {
                for fwd in hostfwds {
                    netdev.push_str(&format!(",hostfwd={}", fwd));
                }
            }
        }
        cmd.arg("-netdev").arg(netdev);

        let mut device = String::from("virtio-net-device,netdev=net0,bus=virtio-mmio-bus.1");
        if let Some(mac) = &cfg.qemu.mac {
            device.push_str(&format!(",mac={}", mac));
        }
        cmd.arg("-device").arg(device);
    }

    cmd.arg("-initrd").arg("target/modules.bin");
    if let Some(args) = &cfg.qemu.bootargs {
        cmd.arg("-append").arg(args);
    }
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

pub fn qemu_dump_dtb(cfg: &Config) -> anyhow::Result<()> {
    let qemu = qemu_cmd(cfg)?;
    let mut cmd = Command::new(&qemu);
    let dtb_path = "target/virt.dtb";

    cmd.arg("-machine").arg(format!("virt,dumpdtb={}", dtb_path));
    // CPUs
    if cfg.qemu.cpus > 1 {
        cmd.arg("-smp").arg(cfg.qemu.cpus.to_string());
    }
    // Memory
    cmd.arg("-m").arg(&cfg.qemu.mem);
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
