use crate::config::Config;
use crate::util::run;
use std::fs;
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

    if !cfg.system.arch.qemu_machine().is_empty() {
        cmd.arg("-machine").arg(cfg.system.arch.qemu_machine());
    }

    // OpenSBI 直启内核时不挂载 EFI/vvfat 启动盘。
    // 其它引导模式保留 vvfat 启动盘，便于加载 EFI/boot 产物。
    let mut next_mmio_bus = 0usize;
    cmd.arg("-drive").arg(format!("file=fat:rw:{},format=raw,if=none,id=boot", fsroot.display()));
    let mut boot_device = cfg.system.arch.qemu_block_device("boot", next_mmio_bus);
    boot_device.push_str(",bootindex=0");
    cmd.arg("-device").arg(boot_device);
    if cfg.system.arch.uses_mmio_virtio() {
        next_mmio_bus += 1;
    }

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
        crate::arch::Bootloader::Linuxboot => {
            // 直接将内核作为 Payload (Linux boot protocol)
            let mut kernel_path = std::env::current_dir()?.join("target/kernel");

            // AArch64 QEMU virt doesn't boot ELF via -kernel correctly, use raw binary.
            if cfg.system.arch == crate::arch::Arch::Aarch64 {
                cmd.arg("-cpu").arg("max");

                let bin_path = std::env::current_dir()?.join("target/kernel.bin");
                let objcopy = cfg.system.arch.llvm_tool("objcopy");
                let status = Command::new(objcopy)
                    .args(&[
                        "-O",
                        "binary",
                        kernel_path.to_str().unwrap(),
                        bin_path.to_str().unwrap(),
                    ])
                    .status()?;
                if !status.success() {
                    return Err(anyhow::anyhow!(
                        "[ ERROR ] Failed to convert kernel ELF to binary"
                    ));
                }
                kernel_path = bin_path;
            }

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
                let vars_candidates = cfg.system.arch.uefi_vars_candidates();

                if let Some(path) = candidates.iter().find(|p| PathBuf::from(p).exists()) {
                    eprintln!("[ INFO ] Using UEFI Firmware: {}", path);
                    cmd.arg("-drive")
                        .arg(format!("if=pflash,format=raw,unit=0,file={},readonly=on", path));
                    if let Some(vars_source) =
                        vars_candidates.iter().find(|p| PathBuf::from(p).exists())
                    {
                        let vars_copy = std::env::current_dir()?
                            .join(format!("target/{}-vars.fd", cfg.system.arch.as_str()));
                        fs::copy(vars_source, &vars_copy)?;
                        cmd.arg("-drive").arg(format!(
                            "if=pflash,format=raw,unit=1,file={}",
                            vars_copy.display()
                        ));
                    }
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
            cmd.arg("-display").arg("none");
        } else if *display == "none" {
            cmd.arg("-display").arg("none");
        } else {
            // Add GPU device for graphical output
            cmd.arg("-device").arg(cfg.system.arch.qemu_gpu_device());
            // Add input devices
            for device in cfg.system.arch.qemu_input_devices() {
                cmd.arg("-device").arg(device);
            }
            // cmd.arg("-device").arg("virtio-tablet-pci");

            cmd.arg("-display").arg(display);
        }
    }

    let serial_port = cfg.qemu.serial_port.unwrap_or(5555);
    // serial0: debug/output channel bound to stdio
    cmd.arg("-serial").arg("stdio");
    // serial1: guest-enumerable UART via PCI serial controller
    cmd.arg("-chardev").arg(format!(
        "socket,id=uart1,host=127.0.0.1,port={serial_port},server=on,wait=off,telnet=on"
    ));
    cmd.arg("-device").arg("pci-serial,chardev=uart1");

    let data_disk = cfg.qemu.disk.as_ref();
    if let Some(disk) = data_disk {
        if !disk.is_empty() {
            cmd.arg("-drive").arg(format!("file={disk},if=none,format=raw,id=disk0"));
            cmd.arg("-device").arg(cfg.system.arch.qemu_block_device("disk0", next_mmio_bus));
            if cfg.system.arch.uses_mmio_virtio() {
                next_mmio_bus += 1;
            }
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

        let mut device = cfg.system.arch.qemu_net_device("net0", next_mmio_bus).into_owned();
        if let Some(mac) = &cfg.qemu.mac {
            device.push_str(&format!(",mac={}", mac));
        }
        cmd.arg("-device").arg(device);
    }

    // Use vvfat for the boot device
    // Removed duplicate boot drive addition

    // Add telnet for debugging
    cmd.arg("-monitor").arg("telnet:127.0.0.1:45454,server,nowait");

    if cfg.system.arch.uses_mmio_virtio() {
        // Force modern virtio transport on MMIO platforms.
        cmd.arg("-global").arg("virtio-mmio.force-legacy=false");
    }
    // Debug options
    cmd.arg("-d").arg("int,guest_errors,cpu_reset");
    cmd.arg("-D").arg("qemu.log");
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
    cmd.arg("-machine").arg("virt");
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
