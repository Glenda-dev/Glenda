use acpi::{AcpiTables, Handler, PhysicalMapping};
use core::ptr::NonNull;

#[derive(Clone, Copy)]
pub struct GlendaAcpiHandler;

impl Handler for GlendaAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        unsafe {
            PhysicalMapping {
                physical_start: physical_address,
                virtual_start: NonNull::new_unchecked(physical_address as *mut T),
                region_length: size,
                mapped_length: size,
                handler: *self,
            }
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {
        // Nothing to do for identity mapping
    }

    fn read_u8(&self, _address: usize) -> u8 {
        todo!()
    }
    fn read_u16(&self, _address: usize) -> u16 {
        todo!()
    }
    fn read_u32(&self, _address: usize) -> u32 {
        todo!()
    }
    fn read_u64(&self, _address: usize) -> u64 {
        todo!()
    }
    fn write_u8(&self, _address: usize, _value: u8) {
        todo!()
    }
    fn write_u16(&self, _address: usize, _value: u16) {
        todo!()
    }
    fn write_u32(&self, _address: usize, _value: u32) {
        todo!()
    }
    fn write_u64(&self, _address: usize, _value: u64) {
        todo!()
    }
    fn read_io_u8(&self, _port: u16) -> u8 {
        todo!()
    }
    fn read_io_u16(&self, _port: u16) -> u16 {
        todo!()
    }
    fn read_io_u32(&self, _port: u16) -> u32 {
        todo!()
    }
    fn write_io_u8(&self, _port: u16, _value: u8) {
        todo!()
    }
    fn write_io_u16(&self, _port: u16, _value: u16) {
        todo!()
    }
    fn write_io_u32(&self, _port: u16, _value: u32) {
        todo!()
    }
    fn read_pci_u8(&self, _address: acpi::PciAddress, _offset: u16) -> u8 {
        todo!()
    }
    fn read_pci_u16(&self, _address: acpi::PciAddress, _offset: u16) -> u16 {
        todo!()
    }
    fn read_pci_u32(&self, _address: acpi::PciAddress, _offset: u16) -> u32 {
        todo!()
    }
    fn write_pci_u8(&self, _address: acpi::PciAddress, _offset: u16, _value: u8) {
        todo!()
    }
    fn write_pci_u16(&self, _address: acpi::PciAddress, _offset: u16, _value: u16) {
        todo!()
    }
    fn write_pci_u32(&self, _address: acpi::PciAddress, _offset: u16, _value: u32) {
        todo!()
    }
    fn nanos_since_boot(&self) -> u64 {
        0
    }
    fn stall(&self, _nanoseconds: u64) {}
    fn sleep(&self, _milliseconds: u64) {}
}

pub fn parse(rsdp_addr: usize) {
    let handler = GlendaAcpiHandler;

    let tables =
        unsafe { AcpiTables::from_rsdp(handler, rsdp_addr).expect("Failed to parse ACPI tables") };

    crate::hal::platform::parse_acpi(&tables);
}
