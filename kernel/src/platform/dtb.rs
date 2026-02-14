use crate::hal;
use fdt::Fdt;

pub fn parse(dtb_ptr: usize) {
    let fdt = unsafe { Fdt::from_ptr(dtb_ptr as *const u8).expect("Failed to parse FDT") };

    hal::platform::parse_dtb(&fdt);
}
