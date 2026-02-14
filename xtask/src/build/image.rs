use crate::arch::Arch;
use crate::config::Config;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn prepare(cfg: &Config) -> anyhow::Result<()> {
    // Copy kernel to top level target similar to build_kernel
    // Note: build_kernel copies to target/kernel already.

    let fsroot = Path::new("target/fsroot");
    let limine_path = Path::new("target/limine");

    if fsroot.exists() {
        fs::remove_dir_all(fsroot)?;
    }
    fs::create_dir_all(fsroot.join("EFI/BOOT"))?;
    fs::create_dir_all(fsroot.join("boot"))?;

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
    fs::copy("target/kernel", fsroot.join("boot/glenda.elf"))?;
    if Path::new("target/modules.bin").exists() {
        fs::copy("target/modules.bin", fsroot.join("boot/modules.bin"))?;
    } else {
        // Create empty? or warn
        eprintln!("[ WARN ] No modules.bin found");
    }

    // Copy Limine files (UEFI)
    let arch = cfg.system.arch;
    let limine_efi = arch.limine_efi_file();
    fs::copy(limine_path.join(limine_efi), fsroot.join("EFI/BOOT").join(limine_efi))?;

    // Copy CD/BIOS helper files
    fs::copy(limine_path.join("limine-uefi-cd.bin"), fsroot.join("boot/limine-uefi-cd.bin"))?;
    if matches!(arch, Arch::X86_64) {
        fs::copy(limine_path.join("limine-bios.sys"), fsroot.join("boot/limine-bios.sys"))?;
        fs::copy(limine_path.join("limine-bios-cd.bin"), fsroot.join("boot/limine-bios-cd.bin"))?;
    }

    let limine_conf_path = Path::new("config/limine.conf");
    fs::copy(limine_conf_path, fsroot.join("boot/limine.conf"))?;
    eprintln!("[ INFO ] Preparation complete. Files copied to {}", fsroot.display());
    Ok(())
}

pub fn image_img(cfg: &Config) -> anyhow::Result<()> {
    // Copy kernel to top level target similar to build_kernel
    // Note: build_kernel copies to target/kernel already.
    let arch = cfg.system.arch;
    let limine_path = Path::new("target/limine");
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
    let limine_efi = arch.limine_efi_file();
    mcopy(&limine_path.join(limine_efi), &format!("EFI/BOOT/{}", limine_efi))?;

    // Write limine.conf to temp file then copy
    // (We reuse the previous limine.conf creation logic, assuming it's written to target/fsroot/boot/limine.conf)
    // Actually we can just write it to a temp path.
    // Let's use target/limine.conf as temp
    mcopy(limine_conf_path, "boot/limine.conf")?;

    eprintln!("[ INFO ] Image generated at {}", image_path.display());

    Ok(())
}

pub fn image_iso(_cfg: &Config) -> anyhow::Result<()> {
    let fsroot = Path::new("target/fsroot");
    let iso_path = Path::new("target/glenda.iso");

    eprintln!("[ INFO ] Generating ISO image...");

    let status = Command::new("xorriso")
        .args(&[
            "-as",
            "mkisofs",
            "-b",
            "boot/limine-uefi-cd.bin",
            "-no-emul-boot",
            "-boot-load-size",
            "4",
            "-boot-info-table",
            "--efi-boot",
            "boot/limine-uefi-cd.bin",
            "-efi-boot-part",
            "--efi-boot-image",
            "--protective-msdos-label",
            fsroot.to_str().unwrap(),
            "-o",
            iso_path.to_str().unwrap(),
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("xorriso failed");
    }

    Ok(())
}
