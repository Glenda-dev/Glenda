#![no_std]
#![no_main]

extern crate alloc;
use core::ptr::NonNull;
use glenda::cap::CapPtr;
use glenda::protocol::unicorn::*;
use virtio_common::{consts::*, VirtIOError, VirtIOTransport};

struct VirtIOBlk {
    transport: VirtIOTransport,
}

impl VirtIOBlk {
    unsafe fn new(base: NonNull<u8>) -> Result<Self, VirtIOError> {
        let transport = VirtIOTransport::new(base)?;
        Ok(Self { transport })
    }

    fn init(&mut self) -> Result<(), VirtIOError> {
        // 1. Reset
        self.transport.set_status(0);

        // 2. Set ACKNOWLEDGE status bit
        self.transport.add_status(STATUS_ACKNOWLEDGE);

        // 3. Set DRIVER status bit
        self.transport.add_status(STATUS_DRIVER);

        // 4. Negotiate features
        let features = self.transport.get_features();
        // Enable VIRTIO_BLK_F_SIZE_MAX etc if needed
        self.transport.set_features(features);

        // 5. Set FEATURES_OK status bit
        self.transport.add_status(STATUS_FEATURES_OK);
        if self.transport.get_status() & STATUS_FEATURES_OK == 0 {
            return Err(VirtIOError::DeviceNotFound); // Feature negotiation failed
        }

        // 6. Setup queues
        // ... (Requires DMA allocation)

        // 7. Set DRIVER_OK status bit
        self.transport.add_status(STATUS_DRIVER_OK);

        Ok(())
    }
}

#[no_mangle]
fn main() -> ! {
    // 1. Get Device Capability
    let device_cap = CapPtr::new(10);

    // 2. Map MMIO (Mocked for now)
    // let mmio_cap = device_cap.invoke(MAP_MMIO, ...);
    // let mmio_ptr = vm::map(mmio_cap);
    let mmio_ptr = 0x1000_1000 as *mut u8; // Mock

    let mut driver = unsafe {
        VirtIOBlk::new(NonNull::new(mmio_ptr).unwrap()).expect("Failed to init virtio-blk")
    };

    driver.init().expect("Failed to init device");

    let tcb_cap = CapPtr::new(2); // TCB is usually in slot 2
    loop {
        tcb_cap.tcb_suspend();
        // Wait for IRQ
    }
}
