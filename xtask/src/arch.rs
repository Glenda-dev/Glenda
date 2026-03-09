use clap::ValueEnum;
use serde::Deserialize;
use std::fmt::{self, Display, Formatter};

#[derive(ValueEnum, Clone, Copy, Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    #[default]
    Riscv64,
    #[serde(rename = "riscv32")]
    Riscv32,
    X86_64,
    Aarch64,
    Loongarch64,
    Hosted,
}

impl Arch {
    pub fn as_str(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "riscv64",
            Arch::Riscv32 => "riscv32",
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
            Arch::Loongarch64 => "loongarch64",
            Arch::Hosted => "hosted",
        }
    }

    pub fn target_triple(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "riscv64gc-unknown-none-elf",
            Arch::Riscv32 => "riscv32imac-unknown-none-elf",
            Arch::X86_64 => "x86_64-unknown-none",
            Arch::Aarch64 => "aarch64-unknown-none-elf",
            Arch::Loongarch64 => "loongarch64-unknown-none",
            Arch::Hosted => "hosted",
        }
    }

    pub fn qemu_binary(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "qemu-system-riscv64",
            Arch::Riscv32 => "qemu-system-riscv32",
            Arch::X86_64 => "qemu-system-x86_64",
            Arch::Aarch64 => "qemu-system-aarch64",
            Arch::Loongarch64 => "qemu-system-loongarch64",
            Arch::Hosted => "none",
        }
    }

    pub fn binutils_prefix(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "riscv64-unknown-elf-",
            Arch::Riscv32 => "riscv32-unknown-elf-",
            Arch::X86_64 => "x86_64-elf-",
            Arch::Aarch64 => "aarch64-elf-",
            Arch::Loongarch64 => "loongarch64-elf-",
            Arch::Hosted => "",
        }
    }

    pub fn uefi_firmware_candidates(&self) -> Vec<String> {
        let local = format!("firmware/{}_uefi.fd", self.as_str());
        match self {
            Arch::Riscv64 => vec![local, "/usr/share/edk2/riscv/RISCV_VIRT_CODE.fd".to_string()],
            Arch::Riscv32 => vec![],
            Arch::Aarch64 => vec![local, "/usr/share/edk2/aarch64/QEMU_EFI.fd".to_string()],
            Arch::X86_64 => vec![
                local,
                "/usr/share/ovmf/X64/OVMF.fd".to_string(),
                "/usr/share/qemu/OVMF.fd".to_string(),
                "/usr/share/ovmf/ovmf_code_x64.bin".to_string(),
            ],
            Arch::Loongarch64 => vec![local, "/usr/share/edk2/loongarch/QEMU_EFI.fd".to_string()],
            Arch::Hosted => vec![],
        }
    }

    pub fn uefi_firmware_url(&self) -> Option<&'static str> {
        match self {
            Arch::Riscv32 => None,
            Arch::Riscv64 => {
                Some("https://github.com/qemu/qemu/raw/master/pc-bios/edk2-riscv64-code.fd.bz2")
            }
            Arch::Aarch64 => {
                Some("https://github.com/qemu/qemu/raw/master/pc-bios/edk2-aarch64-code.fd.bz2")
            }
            Arch::X86_64 => {
                Some("https://github.com/qemu/qemu/raw/master/pc-bios/edk2-x86_64-code.fd.bz2")
            }
            Arch::Loongarch64 => {
                Some("https://github.com/qemu/qemu/raw/master/pc-bios/edk2-loongarch64-code.fd.bz2")
            }
            Arch::Hosted => None,
        }
    }

    pub fn limine_efi_file(&self) -> &'static str {
        match self {
            Arch::Riscv32 => "BOOTRISCV32.EFI",
            Arch::Riscv64 => "BOOTRISCV64.EFI",
            Arch::X86_64 => "BOOTX64.EFI",
            Arch::Aarch64 => "BOOTAA64.EFI",
            Arch::Loongarch64 => "BOOTLOONGARCH64.EFI",
            Arch::Hosted => "",
        }
    }
}

impl Display for Arch {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Bootloader {
    #[default]
    Limine,
    Uboot,
    Opensbi,
    Multiboot2,
    Uefi,
}

impl Bootloader {
    pub fn as_str(&self) -> &'static str {
        match self {
            Bootloader::Limine => "limine",
            Bootloader::Uboot => "uboot",
            Bootloader::Opensbi => "opensbi",
            Bootloader::Multiboot2 => "multiboot2",
            Bootloader::Uefi => "uefi",
        }
    }
}
