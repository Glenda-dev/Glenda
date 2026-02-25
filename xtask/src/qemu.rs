use crate::config::Config;
use crate::util::run;
use std::path::PathBuf;
use std::process::Command;
use which::which;

pub fn qemu_cmd(cfg: &Config) -> anyhow::Result<Command> {
    let qemu_arch = cfg.system.arch.qemu_binary();
    let qemu = which(qemu_arch)
        .map_err(|_| anyhow::anyhow!("[ ERROR ] {} not found in PATH", qemu_arch))?;

    let mut cmd = Command::new(&qemu);

    // Use vvfat for the boot directory to allow easy updates
    // Map target/fsroot to a virtual FAT drive
    let fsroot = std::env::current_dir()?.join("target/fsroot");
    if !fsroot.exists() {
        return Err(anyhow::anyhow!(
            "[ ERROR ] target/fsroot not found. Run `cargo xtask build` first."
        ));
    }

    cmd.arg("-machine").arg("virt,acpi=on");

    // Use vvfat for the boot device, place it BEFORE other drives so it's virtio 0
    cmd.arg("-drive").arg(format!("file=fat:rw:{},format=raw,if=none,id=boot", fsroot.display()));
    cmd.arg("-device").arg("virtio-blk-device,drive=boot");

    match cfg.system.bootloader {
        crate::arch::Bootloader::Uboot => {
            // Use U-Boot as the primary "kernel" (payload for OpenSBI)
            let uboot_path = if let Some(bios) = &cfg.qemu.bios {
                PathBuf::from(bios)
            } else {
                std::env::current_dir()?
                    .join(format!("firmware/u-boot_{}.bin", cfg.system.arch.as_str()))
            };

            if !uboot_path.exists() {
                return Err(anyhow::anyhow!(
                    "[ ERROR ] U-Boot binary not found at {}\n[ HELP  ] Please place the U-Boot binary there or specify `bios` in config.toml.",
                    uboot_path.display()
                ));
            }

            // Pass U-Boot as -kernel to QEMU. It will be loaded after OpenSBI.
            cmd.arg("-kernel").arg(uboot_path);
        }
        crate::arch::Bootloader::Opensbi => {
            // 直接将内核作为 OpenSBI 的 Payload
            let kernel_path = std::env::current_dir()?.join("target/kernel");
            cmd.arg("-kernel").arg(kernel_path);

            // 指定 initrd 为 modules.bin
            let initrd_path = fsroot.join("boot/modules.bin");
            if initrd_path.exists() {
                cmd.arg("-initrd").arg(initrd_path);
            }
        }
        crate::arch::Bootloader::Limine
        | crate::arch::Bootloader::Multiboot2
        | crate::arch::Bootloader::Uefi => {
            // BIOS/Firmware handling (OVMF/EDK2)
            if let Some(bios) = &cfg.qemu.bios {
                // User provided specific BIOS path
                if PathBuf::from(bios).exists() {
                    cmd.arg("-drive")
                        .arg(format!("if=pflash,format=raw,unit=0,file={},readonly=on", bios));
                } else {
                    // Maybe it's just a keyword or QEMU resource name
                    cmd.arg("-bios").arg(bios);
                }
            } else {
                let candidates = cfg.system.arch.uefi_firmware_candidates();

                if let Some(path) = candidates.iter().find(|p| PathBuf::from(p).exists()) {
                    eprintln!("[ INFO ] Using UEFI Firmware: {}", path);
                    cmd.arg("-drive")
                        .arg(format!("if=pflash,format=raw,unit=0,file={},readonly=on", path));
                } else {
                    // Try to download
                    if let Some(url) = cfg.system.arch.uefi_firmware_url() {
                        let local = std::env::current_dir()?.join(&candidates[0]);
                        crate::util::download(url, &local)?;
                        cmd.arg("-drive").arg(format!(
                            "if=pflash,format=raw,unit=0,file={},readonly=on",
                            local.display()
                        ));
                    } else {
                        return Err(anyhow::anyhow!(
                        "[ ERROR ] No UEFI firmware found for {}. Please install edk2/ovmf or specify `bios` in config.toml.",
                        cfg.system.arch.as_str()
                    ));
                    }
                }
            }
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
        // Skip if drive is already disk.img which might be same as boot
        if drive != "disk.img" {
            cmd.arg("-drive").arg(format!("file={drive},if=none,format=raw,id=x0"));
            cmd.arg("-device").arg("virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0");
        }
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

    // Use vvfat for the boot device
    // Removed duplicate boot drive addition

    // Add telnet for debugging
    cmd.arg("-monitor").arg("telnet:127.0.0.1:45454,server,nowait");

    // Force Modern VirtIOg
    cmd.arg("-global").arg("virtio-mmio.force-legacy=false");

    Ok(cmd)
}

pub fn qemu_run(cfg: &Config, timeout: Option<u64>) -> anyhow::Result<()> {
    let mut cmd = qemu_cmd(cfg)?;
    crate::util::run_with_timeout(&mut cmd, timeout)
}

pub fn qemu_gdb(cfg: &Config, port: u16) -> anyhow::Result<()> {
    let mut cmd = qemu_cmd(&cfg)?;
    cmd.arg("-gdb").arg(format!("tcp::{port}"));
    cmd.arg("-S");
    eprintln!("QEMU started. In another shell:");
    run(&mut cmd)
}

pub fn qemu_dump_dtb(cfg: &Config) -> anyhow::Result<()> {
    let mut cmd = qemu_cmd(cfg)?;
    let dtb_path = "target/virt.dtb";

    cmd.arg("-machine").arg(format!("virt,dumpdtb={}", dtb_path));
    eprintln!("[ INFO ] Dumping DTB to {}...", dtb_path);
    run(&mut cmd)?;
    eprintln!("[ INFO ] DTB dumped successfully.");
    eprintln!(
        "[ INFO ] You can decompile it with: dtc -I dtb -O dts -o target/virt.dts {}",
        dtb_path
    );
    Ok(())
}

pub fn qemu_dump_acpi(cfg: &Config) -> anyhow::Result<()> {
    // Note: acpi_table_save is currently only supported on x86 and some ARM targets in QEMU.
    // For RISC-V, this command is not yet available in the HMP monitor.
    if cfg.system.arch.as_str() != "x86_64" && cfg.system.arch.as_str() != "aarch64" {
        eprintln!(
            "[ WARN ] `acpi_table_save` is not supported by QEMU for {}.",
            cfg.system.arch.as_str()
        );
        eprintln!("[ INFO ] For RISC-V, you can dump ACPI tables by:");
        eprintln!("  1. Booting into UEFI shell and using the `acpiview` command.");
        eprintln!("  2. Reading the tables directly from the host if they were provided as files.");
        return Ok(());
    }

    let mut cmd = qemu_cmd(cfg)?;
    let acpi_path = std::env::current_dir()?.join("target/acpi.dat");
    let acpi_path_str = acpi_path.to_str().ok_or(anyhow::anyhow!("Invalid path"))?;

    // For RISC-V/ARM virt machine, we need to explicitly enable ACPI
    cmd.arg("-machine").arg("virt,acpi=on");
    cmd.arg("-display").arg("none");
    // Disable default serial to avoid conflict with monitor on stdio
    cmd.arg("-serial").arg("null");
    cmd.arg("-monitor").arg("stdio");
    cmd.arg("-S");

    eprintln!("[ INFO ] Dumping ACPI tables to {}...", acpi_path_str);

    use std::io::Write;
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped()) // Capture output to see if there are errors
        .spawn()
        .map_err(|e| anyhow::anyhow!("[ ERROR ] Failed to start QEMU: {}", e))?;

    let mut stdin = child.stdin.take().unwrap();
    // Wait for QEMU to initialize monitor a bit longer
    std::thread::sleep(std::time::Duration::from_millis(1000));
    writeln!(stdin, "acpi_table_save {}", acpi_path_str)?;
    writeln!(stdin, "quit")?;

    let status = child.wait()?;
    if status.success() {
        eprintln!("[ INFO ] ACPI tables dumped to {}.", acpi_path_str);
        eprintln!(
            "[ INFO ] You can use `acpixtract -a {}` on the host to extract individual tables.",
            acpi_path_str
        );
    } else {
        return Err(anyhow::anyhow!("[ ERROR ] QEMU failed during ACPI dump"));
    }

    Ok(())
}
