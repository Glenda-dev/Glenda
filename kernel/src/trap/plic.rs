#![allow(dead_code)]
use core::ptr::{read_volatile, write_volatile};
use driver_uart::UART_IRQ;

const PLIC_BASE: usize = 0x0C00_0000;

pub fn init() {
    set_priority(UART_IRQ, 1); // 设置 UART 的优先级为 1
}
pub fn init_hart(hartid: usize) {
    set_enable_s(hartid, UART_IRQ, true); // 启用 UART 的中断
    set_priority_s(hartid, UART_IRQ, 0); // 设置当前 hart 的 UART 中断优先级为 1
}
pub fn claim(hartid: usize) -> usize {
    get_claim_s(hartid)
}
pub fn complete(hartid: usize, id: usize) {
    set_claim_s(hartid, id);
}

pub fn set_priority(id: usize, priority: usize) {
    unsafe {
        let addr = PLIC_BASE + id * 4;
        write_volatile(addr as *mut u32, priority as u32);
    }
}

pub fn get_priority(id: usize) -> usize {
    unsafe {
        let addr = PLIC_BASE + id * 4;
        read_volatile(addr as *const u32) as usize
    }
}

pub fn set_enable_m(hartid: usize, id: usize, enable: bool) {
    unsafe {
        let addr = PLIC_BASE + 0x2000 + hartid * 0x100;
        let bit = 1 << (id % 32);
        let mask = bit as u32;
        if enable {
            write_volatile(addr as *mut u32, read_volatile(addr as *const u32) | mask);
        } else {
            write_volatile(addr as *mut u32, read_volatile(addr as *const u32) & !mask);
        }
    }
}

pub fn get_enable_m(hartid: usize, id: usize) -> bool {
    unsafe {
        let addr = PLIC_BASE + 0x2000 + hartid * 0x100;
        let bit = 1 << (id % 32);
        (read_volatile(addr as *const u32) & bit as u32) != 0
    }
}

pub fn set_enable_s(hartid: usize, id: usize, enable: bool) {
    unsafe {
        let addr = PLIC_BASE + 0x2080 + hartid * 0x100;
        let bit = 1 << (id % 32);
        if enable {
            write_volatile(addr as *mut u32, read_volatile(addr as *const u32) | bit as u32);
        } else {
            write_volatile(addr as *mut u32, read_volatile(addr as *const u32) & !(bit as u32));
        }
    }
}
pub fn get_enable_s(hartid: usize, id: usize) -> bool {
    unsafe {
        let addr = PLIC_BASE + 0x2080 + hartid * 0x100;
        let bit = 1 << (id % 32);
        (read_volatile(addr as *const u32) & bit as u32) != 0
    }
}

pub fn set_priority_m(hartid: usize, id: usize, priority: usize) {
    unsafe {
        let addr = PLIC_BASE + 0x2000 + hartid * 0x100 + id * 4;
        write_volatile(addr as *mut u32, priority as u32);
    }
}

pub fn get_priority_m(hartid: usize, id: usize) -> usize {
    unsafe {
        let addr = PLIC_BASE + 0x2000 + hartid * 0x100 + id * 4;
        read_volatile(addr as *const u32) as usize
    }
}

pub fn set_priority_s(hartid: usize, id: usize, priority: usize) {
    unsafe {
        let addr = PLIC_BASE + 0x2080 + hartid * 0x100 + id * 4;
        write_volatile(addr as *mut u32, priority as u32);
    }
}

pub fn get_priority_s(hartid: usize, id: usize) -> usize {
    unsafe {
        let addr = PLIC_BASE + 0x2080 + hartid * 0x100 + id * 4;
        read_volatile(addr as *const u32) as usize
    }
}

pub fn set_claim_m(hartid: usize, id: usize) {
    unsafe {
        let addr = PLIC_BASE + 0x2004 + hartid * 0x100;
        write_volatile(addr as *mut u32, id as u32);
    }
}

pub fn get_claim_m(hartid: usize) -> usize {
    unsafe {
        let addr = PLIC_BASE + 0x2004 + hartid * 0x100;
        read_volatile(addr as *const u32) as usize
    }
}

pub fn set_claim_s(hartid: usize, id: usize) {
    unsafe {
        let addr = PLIC_BASE + 0x2084 + hartid * 0x100;
        write_volatile(addr as *mut u32, id as u32);
    }
}

pub fn get_claim_s(hartid: usize) -> usize {
    unsafe {
        let addr = PLIC_BASE + 0x2084 + hartid * 0x100;
        read_volatile(addr as *const u32) as usize
    }
}
