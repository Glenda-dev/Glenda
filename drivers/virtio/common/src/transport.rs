use super::consts::*;
use super::{Result, VirtIOError};
use core::ptr::NonNull;

pub struct VirtIOTransport {
    base: NonNull<u8>,
}

impl VirtIOTransport {
    pub unsafe fn new(base: NonNull<u8>) -> Result<Self> {
        let mut transport = Self { base };

        if transport.read_reg(OFF_MAGIC) != MAGIC_VALUE {
            return Err(VirtIOError::InvalidHeader);
        }

        if transport.read_reg(OFF_VERSION) != VERSION_2 {
            // For simplicity, we only support V2 (VirtIO 1.0) for now, or handle legacy if needed.
            // But QEMU virt usually provides V2.
        }

        Ok(transport)
    }

    unsafe fn read_reg(&self, offset: usize) -> u32 {
        let ptr = self.base.as_ptr().add(offset) as *const u32;
        core::ptr::read_volatile(ptr)
    }

    unsafe fn write_reg(&self, offset: usize, val: u32) {
        let ptr = self.base.as_ptr().add(offset) as *mut u32;
        core::ptr::write_volatile(ptr, val);
    }

    pub fn get_device_id(&self) -> u32 {
        unsafe { self.read_reg(OFF_DEVICE_ID) }
    }

    pub fn get_vendor_id(&self) -> u32 {
        unsafe { self.read_reg(OFF_VENDOR_ID) }
    }

    pub fn get_status(&self) -> u32 {
        unsafe { self.read_reg(OFF_STATUS) }
    }

    pub fn set_status(&self, status: u32) {
        unsafe { self.write_reg(OFF_STATUS, status) }
    }

    pub fn add_status(&self, status: u32) {
        let old = self.get_status();
        self.set_status(old | status);
    }

    pub fn get_features(&self) -> u64 {
        unsafe {
            self.write_reg(OFF_DEVICE_FEATURES_SEL, 0);
            let low = self.read_reg(OFF_DEVICE_FEATURES);
            self.write_reg(OFF_DEVICE_FEATURES_SEL, 1);
            let high = self.read_reg(OFF_DEVICE_FEATURES);
            ((high as u64) << 32) | (low as u64)
        }
    }

    pub fn set_features(&self, features: u64) {
        unsafe {
            self.write_reg(OFF_DRIVER_FEATURES_SEL, 0);
            self.write_reg(OFF_DRIVER_FEATURES, features as u32);
            self.write_reg(OFF_DRIVER_FEATURES_SEL, 1);
            self.write_reg(OFF_DRIVER_FEATURES, (features >> 32) as u32);
        }
    }

    pub fn notify(&self, queue_idx: u32) {
        unsafe { self.write_reg(OFF_QUEUE_NOTIFY, queue_idx) }
    }

    pub fn ack_interrupt(&self) -> u32 {
        unsafe {
            let status = self.read_reg(OFF_INTERRUPT_STATUS);
            self.write_reg(OFF_INTERRUPT_ACK, status);
            status
        }
    }

    // Queue configuration methods would go here...
    // For brevity, I'll assume the driver handles queue setup using raw writes or helper methods.
    pub unsafe fn write_queue_sel(&self, idx: u32) {
        self.write_reg(OFF_QUEUE_SEL, idx);
    }

    pub unsafe fn read_queue_max(&self) -> u32 {
        self.read_reg(OFF_QUEUE_NUM_MAX)
    }

    pub unsafe fn write_queue_num(&self, num: u32) {
        self.write_reg(OFF_QUEUE_NUM, num);
    }

    pub unsafe fn write_queue_desc(&self, addr: u64) {
        self.write_reg(OFF_QUEUE_DESC_LOW, addr as u32);
        self.write_reg(OFF_QUEUE_DESC_HIGH, (addr >> 32) as u32);
    }

    pub unsafe fn write_queue_driver(&self, addr: u64) {
        self.write_reg(OFF_QUEUE_DRIVER_LOW, addr as u32);
        self.write_reg(OFF_QUEUE_DRIVER_HIGH, (addr >> 32) as u32);
    }

    pub unsafe fn write_queue_device(&self, addr: u64) {
        self.write_reg(OFF_QUEUE_DEVICE_LOW, addr as u32);
        self.write_reg(OFF_QUEUE_DEVICE_HIGH, (addr >> 32) as u32);
    }

    pub unsafe fn write_queue_ready(&self, ready: u32) {
        self.write_reg(OFF_QUEUE_READY, ready);
    }

    pub unsafe fn config_ptr(&self) -> *mut u8 {
        self.base.as_ptr().add(OFF_CONFIG)
    }
}
