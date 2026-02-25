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

    // Copy kernel and initrd
    fs::copy("target/kernel", fsroot.join("boot/glenda.elf"))?;
    if Path::new("target/modules.bin").exists() {
        fs::copy("target/modules.bin", fsroot.join("boot/modules.bin"))?;
    } else {
        // Create empty? or warn
        eprintln!("[ WARN ] No modules.bin found");
    }

    // Copy Limine files (UEFI)
    // Download Limine if missing
    if cfg.system.bootloader == crate::arch::Bootloader::Limine {
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
        let arch = cfg.system.arch;
        let limine_efi = arch.limine_efi_file();
        fs::copy(limine_path.join(limine_efi), fsroot.join("EFI/BOOT").join(limine_efi))?;

        // Copy CD/BIOS helper files
        fs::copy(limine_path.join("limine-uefi-cd.bin"), fsroot.join("boot/limine-uefi-cd.bin"))?;
        if matches!(arch, Arch::X86_64) {
            fs::copy(limine_path.join("limine-bios.sys"), fsroot.join("boot/limine-bios.sys"))?;
            fs::copy(
                limine_path.join("limine-bios-cd.bin"),
                fsroot.join("boot/limine-bios-cd.bin"),
            )?;
        }

        let limine_conf_path = Path::new("config/limine.conf");
        fs::copy(limine_conf_path, fsroot.join("boot/limine.conf"))?;
    }

    if cfg.system.bootloader == crate::arch::Bootloader::Uefi {
        let arch = cfg.system.arch;
        let efi_name = arch.limine_efi_file();
        let src = if Path::new("target/kernel.efi").exists() {
            "target/kernel.efi"
        } else {
            "target/kernel"
        };
        fs::copy(src, fsroot.join("EFI/BOOT").join(efi_name))?;
        eprintln!("[ INFO ] UEFI EFI Stub installed to /EFI/BOOT/{}", efi_name);
    }

    if cfg.system.bootloader == crate::arch::Bootloader::Multiboot2 {
        let grub_conf_path = Path::new("config/grub.cfg");
        if grub_conf_path.exists() {
            fs::create_dir_all(fsroot.join("boot/grub"))?;
            fs::copy(grub_conf_path, fsroot.join("boot/grub/grub.cfg"))?;
        }
    }

    // Copy uEnv.txt for U-Boot from config directory
    if cfg.system.bootloader == crate::arch::Bootloader::Uboot {
        let uenv_source = Path::new("config/uEnv.txt");
        if uenv_source.exists() {
            fs::copy(uenv_source, fsroot.join("uEnv.txt"))?;
        } else {
            eprintln!("[ WARN ] config/uEnv.txt not found, skipping copy");
        }
        // Generate FIT image if bootloader is uboot
        generate_fit_image(cfg)?;
    }

    eprintln!("[ INFO ] Preparation complete. Files copied to {}", fsroot.display());
    Ok(())
}

fn generate_fit_image(cfg: &Config) -> anyhow::Result<()> {
    eprintln!("[ INFO ] Generating FIT image (glenda.itb)...");
    // 1. kernel binary
    let objcopy = format!("{}objcopy", cfg.system.arch.binutils_prefix());
    let status = Command::new(objcopy)
        .args(&["-O", "binary", "target/kernel", "target/kernel.bin"])
        .status()?;
    if !status.success() {
        anyhow::bail!("objcopy failed to generate kernel.bin");
    }

    // 2. generate ITS file
    let arch = match cfg.system.arch {
        crate::arch::Arch::Riscv64 => "riscv",
        crate::arch::Arch::X86_64 => "x86_64",
        crate::arch::Arch::Aarch64 => "arm64",
        crate::arch::Arch::Loongarch64 => "loongarch",
    };

    let its_content = format!(
        r#"/dts-v1/;

/ {{
    description = "Glenda OS FIT image";
    #address-cells = <1>;

    images {{
        kernel {{
            description = "Glenda Kernel";
            data = /incbin/("kernel.bin");
            type = "kernel";
            arch = "{arch}";
            os = "linux";
            compression = "none";
            load = <0x80200000>;
            entry = <0x80200000>;
        }};
        ramdisk {{
            description = "Glenda Initrd";
            data = /incbin/("modules.bin");
            type = "ramdisk";
            arch = "{arch}";
            os = "linux";
            compression = "none";
            load = <0x88000000>;
        }};
    }};

    configurations {{
        default = "conf-1";
        conf-1 {{
            description = "Glenda Default Configuration";
            kernel = "kernel";
            ramdisk = "ramdisk";
        }};
    }};
}};"#
    );
    fs::write("target/glenda.its", its_content)?;

    // 3. run mkimage
    let status =
        Command::new("mkimage").args(&["-f", "target/glenda.its", "target/glenda.itb"]).status()?;
    if !status.success() {
        eprintln!("[ WARN ] mkimage failed. Make sure u-boot-tools is installed.");
        return Ok(());
    }

    // 4. copy to fsroot
    fs::copy("target/glenda.itb", "target/fsroot/boot/glenda.itb")?;

    // 5. generate boot.scr
    // Although conf-1 loads kernel and ramdisk, we MUST pass FDT address as the 3rd argument
    // otherwise U-Boot resets the working FDT to 0 before jumping.
    let boot_script_content = "fdt addr ${fdtcontroladdr}; fdt resize; fatload ${devtype} ${devnum}:${distro_bootpart} 0x84000000 boot/glenda.itb; bootm 0x84000000:kernel 0x84000000:ramdisk ${fdtcontroladdr}\n";
    let boot_txt_path = "target/boot.txt";
    fs::write(boot_txt_path, boot_script_content)?;

    let status = Command::new("mkimage")
        .args(&[
            "-A",
            "riscv",
            "-T",
            "script",
            "-C",
            "none",
            "-n",
            "Glenda Boot Script",
            "-d",
            boot_txt_path,
            "target/boot.scr",
        ])
        .status()?;

    if status.success() {
        fs::copy("target/boot.scr", "target/fsroot/boot.scr")?;
        eprintln!("[ INFO ] boot.scr generated and copied to fsroot.");
    }

    Ok(())
}

pub fn image_img(_cfg: &Config) -> anyhow::Result<()> {
    // Build Disk Image (FAT32)
    eprintln!("[ INFO ] Generating Disk image (FAT32)...");
    let image_path = Path::new("target/disk.img");
    let fsroot = Path::new("target/fsroot");

    if !fsroot.exists() {
        anyhow::bail!("fsroot does not exist. Run prepare first.");
    }

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
    // mcopy everything from fsroot to root of disk.
    let mut cmd = Command::new("mcopy");
    cmd.arg("-i").arg("target/disk.img").arg("-s").arg("-D").arg("o");

    for entry in std::fs::read_dir(fsroot)? {
        let entry = entry?;
        cmd.arg(entry.path());
    }
    cmd.arg("::/");

    let status = cmd.status()?;

    if !status.success() {
        anyhow::bail!("mcopy failed to populate some files");
    }

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
