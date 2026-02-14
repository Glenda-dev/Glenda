use clap::ValueEnum;
use serde::Deserialize;
use std::fmt::{self, Display, Formatter};

#[derive(ValueEnum, Clone, Copy, Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    #[default]
    Riscv64,
    X86_64,
    Aarch64,
    Loongarch64,
}

impl Arch {
    pub fn as_str(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "riscv64",
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
            Arch::Loongarch64 => "loongarch64",
        }
    }

    pub fn target_triple(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "riscv64gc-unknown-none-elf",
            Arch::X86_64 => "x86_64-unknown-none",
            Arch::Aarch64 => "aarch64-unknown-none-elf",
            Arch::Loongarch64 => "loongarch64-unknown-none",
        }
    }

    pub fn qemu_binary(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "qemu-system-riscv64",
            Arch::X86_64 => "qemu-system-x86_64",
            Arch::Aarch64 => "qemu-system-aarch64",
            Arch::Loongarch64 => "qemu-system-loongarch64",
        }
    }

    pub fn binutils_prefix(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "riscv64-unknown-elf-",
            Arch::X86_64 => "x86_64-elf-",
            Arch::Aarch64 => "aarch64-elf-",
            Arch::Loongarch64 => "loongarch64-elf-",
        }
    }

    pub fn uefi_firmware_candidates(&self) -> Vec<&'static str> {
        match self {
            Arch::Riscv64 => vec!["/usr/share/edk2/riscv/RISCV_VIRT_CODE.fd"],
            Arch::Aarch64 => vec!["/usr/share/edk2/aarch64/QEMU_EFI.fd"],
            Arch::X86_64 => vec!["/usr/share/ovmf/X64/OVMF.fd", "/usr/share/qemu/OVMF.fd"],
            Arch::Loongarch64 => vec!["/usr/share/edk2/loongarch/QEMU_EFI.fd"],
        }
    }

    pub fn limine_efi_file(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "BOOTRISCV64.EFI",
            Arch::X86_64 => "BOOTX64.EFI",
            Arch::Aarch64 => "BOOTAA64.EFI",
            Arch::Loongarch64 => "BOOTLOONGARCH64.EFI",
        }
    }
}

impl Display for Arch {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
