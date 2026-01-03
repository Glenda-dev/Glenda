#![no_std]
#![no_main]
#![allow(dead_code)]

extern crate alloc;
use glenda::cap::CapPtr;
use glenda::protocol::unicorn::*;

// Registers
const RBR: usize = 0;
const THR: usize = 0;
const IER: usize = 1;
const IIR: usize = 2;
const FCR: usize = 2;
const LCR: usize = 3;
const MCR: usize = 4;
const LSR: usize = 5;
const MSR: usize = 6;
const SCR: usize = 7;

struct Ns16550a {
    base: usize,
    irq: CapPtr,
}

impl Ns16550a {
    fn new(base: usize, irq: CapPtr) -> Self {
        Self { base, irq }
    }

    unsafe fn read(&self, offset: usize) -> u8 {
        let ptr = (self.base + offset) as *const u8;
        core::ptr::read_volatile(ptr)
    }

    unsafe fn write(&self, offset: usize, val: u8) {
        let ptr = (self.base + offset) as *mut u8;
        core::ptr::write_volatile(ptr, val);
    }

    fn init(&self) {
        unsafe {
            self.write(IER, 0x00); // Disable interrupts
            self.write(LCR, 0x80); // Enable DLAB
            self.write(0, 0x03); // Divisor low (38.4K)
            self.write(1, 0x00); // Divisor high
            self.write(LCR, 0x03); // 8 bits, no parity, one stop bit
            self.write(FCR, 0x07); // Enable FIFO, clear them
            self.write(MCR, 0x0B); // IRQs enabled, RTS/DSR set
            self.write(IER, 0x01); // Enable RX interrupt
        }
    }

    fn handle_irq(&self) {
        unsafe {
            let iir = self.read(IIR);
            if iir & 0x01 == 0 {
                // Interrupt pending
                if let Some(c) = self.getchar() {
                    // Echo back
                    self.putchar(c);
                }
            }
        }
    }

    unsafe fn getchar(&self) -> Option<u8> {
        if self.read(LSR) & 0x01 != 0 {
            Some(self.read(RBR))
        } else {
            None
        }
    }

    unsafe fn putchar(&self, c: u8) {
        while self.read(LSR) & 0x20 == 0 {}
        self.write(THR, c);
    }
}

#[no_mangle]
fn main() -> ! {
    // 1. Get Device CapPtr (Passed as argument or fixed slot?)
    // Let's assume it's in slot 10 for now.
    let device_cap = CapPtr::new(10);
    let tcb_cap = CapPtr::new(2); // TCB is usually in slot 2
                                  // 2. Map MMIO
                                  // Call Unicorn to get MMIO frame
                                  // ...

    loop {
        tcb_cap.tcb_suspend();
        // Wait for IRQ
    }
}
