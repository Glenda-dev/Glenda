use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use core::ptr::NonNull;

#[derive(Clone, Copy)]
pub struct GlendaAcpiHandler;

impl AcpiHandler for GlendaAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        unsafe {
            PhysicalMapping::new(
                physical_address,
                NonNull::new_unchecked(physical_address as *mut T),
                size,
                size,
                *self,
            )
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {
        // Nothing to do for identity mapping
    }
}

pub fn parse(rsdp_addr: usize) {
    let handler = GlendaAcpiHandler;

    let tables =
        unsafe { AcpiTables::from_rsdp(handler, rsdp_addr).expect("Failed to parse ACPI tables") };

    crate::hal::platform::parse_acpi(&tables);
}
