use crate::arch::Arch;
use crate::config::Config;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn image(cfg: &Config) -> anyhow::Result<()> {
    // Copy kernel to top level target similar to build_kernel
    // Note: build_kernel copies to target/kernel already.

    let img_root = Path::new("target/img_root");
    let limine_path = Path::new("target/limine");

    if img_root.exists() {
        fs::remove_dir_all(img_root)?;
    }
    fs::create_dir_all(img_root.join("EFI/BOOT"))?;
    fs::create_dir_all(img_root.join("boot"))?;

    // Download Limine if missing
    if !limine_path.exists() {
        eprintln!("[ INFO ] Downloading Limine binaries (v10.x-binary)...");
        Command::new("git")
            .args(&[
                "clone",
                "https://github.com/limine-bootloader/limine.git",
                "--branch",
                "v10.x-binary",
                "--depth=1",
                "target/limine",
            ])
            .status()?;
    }

    // Copy kernel and initrd
    fs::copy("target/kernel", img_root.join("boot/glenda.elf"))?;
    if Path::new("target/modules.bin").exists() {
        fs::copy("target/modules.bin", img_root.join("boot/modules.bin"))?;
    } else {
        // Create empty? or warn
        eprintln!("[ WARN ] No modules.bin found");
    }

    // Copy Limine files (UEFI)
    let arch = cfg.system.arch;
    match arch {
        Arch::Riscv64 => {
            fs::copy(
                limine_path.join("BOOTRISCV64.EFI"),
                img_root.join("EFI/BOOT/BOOTRISCV64.EFI"),
            )?;
            fs::copy(
                limine_path.join("limine-uefi-cd.bin"),
                img_root.join("boot/limine-uefi-cd.bin"),
            )?;
        }
        Arch::X86_64 => {
            fs::copy(limine_path.join("BOOTX64.EFI"), img_root.join("EFI/BOOT/BOOTX64.EFI"))?;
            fs::copy(limine_path.join("limine-bios.sys"), img_root.join("boot/limine-bios.sys"))?;
            fs::copy(
                limine_path.join("limine-bios-cd.bin"),
                img_root.join("boot/limine-bios-cd.bin"),
            )?;
            fs::copy(
                limine_path.join("limine-uefi-cd.bin"),
                img_root.join("boot/limine-uefi-cd.bin"),
            )?;
        }
        Arch::Loongarch64 => {
            fs::copy(
                limine_path.join("BOOTLOONGARCH64.EFI"),
                img_root.join("EFI/BOOT/BOOTLOONGARCH64.EFI"),
            )?;
            fs::copy(
                limine_path.join("limine-uefi-cd.bin"),
                img_root.join("boot/limine-uefi-cd.bin"),
            )?;
        }
        Arch::Aarch64 => {
            fs::copy(limine_path.join("BOOTAA64.EFI"), img_root.join("EFI/BOOT/BOOTAA64.EFI"))?;
            fs::copy(
                limine_path.join("limine-uefi-cd.bin"),
                img_root.join("boot/limine-uefi-cd.bin"),
            )?;
        }
    }

    let limine_conf_path = Path::new("config/limine.conf");

    // Build Disk Image (FAT32)
    eprintln!("[ INFO ] Generating Disk image (FAT32)...");
    let image_path = Path::new("target/disk.img");
    if image_path.exists() {
        fs::remove_file(image_path)?;
    }

    // 1. Create empty file (64MB)
    let status = Command::new("dd")
        .args(&["if=/dev/zero", "of=target/disk.img", "bs=1M", "count=64"])
        .status()?;
    if !status.success() {
        anyhow::bail!("dd failed");
    }

    // 2. Format as FAT32
    let status = Command::new("mkfs.fat").args(&["-F", "32", "target/disk.img"]).status()?;
    if !status.success() {
        anyhow::bail!("mkfs.fat failed");
    }

    // 3. Populate using mtools
    // Helper closure
    let mmd = |dir: &str| -> anyhow::Result<()> {
        let status = Command::new("mmd")
            .arg("-i")
            .arg("target/disk.img")
            .arg(format!("::{}", dir))
            .status()?;
        if !status.success() {
            anyhow::bail!("mmd failed for {}", dir);
        }
        Ok(())
    };
    let mcopy = |src: &Path, dst: &str| -> anyhow::Result<()> {
        let status = Command::new("mcopy")
            .arg("-i")
            .arg("target/disk.img")
            .arg(src)
            .arg(format!("::{}", dst))
            .status()?;
        if !status.success() {
            anyhow::bail!("mcopy failed for {}", dst);
        }
        Ok(())
    };

    mmd("EFI")?;
    mmd("EFI/BOOT")?;
    mmd("boot")?;

    // Copy kernel and modules
    mcopy(Path::new("target/kernel"), "boot/glenda.elf")?;
    if Path::new("target/modules.bin").exists() {
        mcopy(Path::new("target/modules.bin"), "boot/modules.bin")?;
    }

    // Copy Limine files
    match arch {
        Arch::Riscv64 => {
            mcopy(&limine_path.join("BOOTRISCV64.EFI"), "EFI/BOOT/BOOTRISCV64.EFI")?;
        }
        Arch::X86_64 => {
            mcopy(&limine_path.join("BOOTX64.EFI"), "EFI/BOOT/BOOTX64.EFI")?;
        }
        Arch::Loongarch64 => {
            mcopy(&limine_path.join("BOOTLOONGARCH64.EFI"), "EFI/BOOT/BOOTLOONGARCH64.EFI")?;
        }
        Arch::Aarch64 => {
            mcopy(&limine_path.join("BOOTAA64.EFI"), "EFI/BOOT/BOOTAA64.EFI")?;
        }
    }

    // Write limine.conf to temp file then copy
    // (We reuse the previous limine.conf creation logic, assuming it's written to target/img_root/boot/limine.conf)
    // Actually we can just write it to a temp path.
    // Let's use target/limine.conf as temp
    mcopy(limine_conf_path, "boot/limine.conf")?;

    eprintln!("[ INFO ] Image generated at {}", image_path.display());

    Ok(())
}
