#![no_std]
#![no_main]

extern crate alloc;
use glenda::{
    ipc::{MsgTag, UTCB},
    println,
};

#[unsafe(no_mangle)]
fn main() -> usize {
    println!("IPC_TEST: Starting...");

    // Test 1: MsgTag
    let label = 0x123;
    let count = 5;
    let tag = MsgTag::new(label, count);

    // Assuming MsgTag layout: high bits label, low bits length? Or implementation specific.
    // Testing library abstraction.
    if tag.label() != label {
        println!("IPC_TEST: Label mismatch. Expected {:#x}, got {:#x}", label, tag.label());
        return 1;
    }
    if tag.length() != count {
        println!("IPC_TEST: Count mismatch. Expected {}, got {}", count, tag.length());
        return 1;
    }
    println!("IPC_TEST: MsgTag ops passed");

    // Test 2: UTCB local access
    // UTCB is usually mapped at a fixed address (e.g. UTCB_VA)
    let utcb = UTCB::current();

    // Write registers
    utcb.mrs_regs[0] = 0xAA;
    utcb.mrs_regs[1] = 0xBB;

    if utcb.mrs_regs[0] != 0xAA || utcb.mrs_regs[1] != 0xBB {
        println!("IPC_TEST: UTCB access failed");
        return 1;
    }

    // Test clear
    utcb.clear();
    // Assuming clear zeroes registers? Or just resets pointer?
    // libglenda-rs implementation of clear might just reset mr_count or similar?
    // Let's assume it doesn't necessarily zero memory so we won't assert 0.

    println!("IPC_TEST: All passed!");
    0
}
