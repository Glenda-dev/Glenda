use crate::mem::pmem;
use crate::mem::pte::perms;
use crate::mem::{PGSIZE, PageTable, PteFlags, VirtAddr};
use core::mem::size_of;

pub const ELF_MAGIC: [u8; 4] = [0x7f, 0x45, 0x4c, 0x46];

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Ehdr {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

pub struct ElfFile<'a> {
    data: &'a [u8],
    header: &'a Elf64Ehdr,
}

impl<'a> ElfFile<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, &'static str> {
        if data.len() < size_of::<Elf64Ehdr>() {
            return Err("Buffer too small for ELF header");
        }
        let header = unsafe { &*(data.as_ptr() as *const Elf64Ehdr) };
        if header.e_ident[0..4] != ELF_MAGIC {
            return Err("Invalid ELF magic");
        }
        Ok(Self { data, header })
    }

    pub fn entry_point(&self) -> usize {
        self.header.e_entry as usize
    }

    pub fn program_headers(&self) -> &'a [Elf64Phdr] {
        let ph_off = self.header.e_phoff as usize;
        let ph_num = self.header.e_phnum as usize;
        let ph_size = self.header.e_phentsize as usize;

        // Safety check
        if ph_off + ph_num * ph_size > self.data.len() {
            return &[];
        }

        unsafe {
            core::slice::from_raw_parts(self.data.as_ptr().add(ph_off) as *const Elf64Phdr, ph_num)
        }
    }

    /// Map LOAD segments into the given page table
    pub fn map(&self, vspace: &mut PageTable) -> Result<(), &'static str> {
        for ph in self.program_headers() {
            if ph.p_type == PT_LOAD {
                let mut flags = PteFlags::from(perms::USER | perms::VALID);
                if ph.p_flags & PF_X != 0 {
                    flags |= perms::EXECUTE;
                }
                if ph.p_flags & PF_W != 0 {
                    flags |= perms::WRITE;
                }
                if ph.p_flags & PF_R != 0 {
                    flags |= perms::READ;
                }
                let start_va = ph.p_vaddr as usize;

                // We need to handle cases where p_vaddr is not page-aligned
                let va_offset = start_va % PGSIZE;
                let aligned_va = start_va - va_offset;

                // Recalculate num_pages based on aligned_va
                let total_memsz = ph.p_memsz as usize + va_offset;
                let num_pages = (total_memsz + PGSIZE - 1) / PGSIZE;

                for j in 0..num_pages {
                    let frame_cap =
                        pmem::alloc_frame_cap(1).ok_or("Failed to alloc frame for segment")?;
                    let va = VirtAddr::from(aligned_va) + j * PGSIZE;

                    // Calculate how much to copy from data
                    let dst_slice = unsafe {
                        core::slice::from_raw_parts_mut(
                            frame_cap.obj_ptr().as_mut_ptr::<u8>(),
                            PGSIZE,
                        )
                    };
                    dst_slice.fill(0);

                    // Calculate source range
                    let page_start_in_segment = if j == 0 { 0 } else { j * PGSIZE - va_offset };
                    let page_end_in_segment = (j + 1) * PGSIZE - va_offset;

                    let copy_start = core::cmp::max(0, page_start_in_segment as isize) as usize;
                    let copy_end = core::cmp::min(ph.p_filesz as usize, page_end_in_segment);

                    if copy_start < copy_end {
                        let src_off = ph.p_offset as usize + copy_start;
                        let dst_off = if j == 0 { va_offset } else { 0 };
                        let len = copy_end - copy_start;

                        dst_slice[dst_off..dst_off + len]
                            .copy_from_slice(&self.data[src_off..src_off + len]);
                    }

                    vspace.map_with_alloc(va, frame_cap.obj_ptr().to_pa(), PGSIZE, flags);
                    core::mem::forget(frame_cap);
                }
            }
        }
        Ok(())
    }
}
