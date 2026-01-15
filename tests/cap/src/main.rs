#![no_std]
#![no_main]

extern crate alloc;
use glenda::{println, cap::{CNode, CapPtr, rights, CSPACE_CAP, CapType, Untyped}};

#[unsafe(no_mangle)]
fn main() -> usize {
    println!("CAP_TEST: Starting...");

    // Test 1: Copy CNode (Root CNode) to a free slot
    // In test_rt, we are given a CSpace. Slots 0-10 are populated?
    // Let's use slot 100 for dest.
    
    let cspace = CSPACE_CAP; // This wrap the Root CNode cap
    let src_cap = cspace.cap(); 
    let dst_slot = 100;

    println!("CAP_TEST: Testing CNode copy...");
    // Note: In libglenda-rs/kernel, copy might return 0 on success.
    
    // We can't easily verify success without invoking the new cap or inspecting CSpace state (which we can't do easily).
    // But if it doesn't panic or return error code, good.
    
    match cspace.copy(src_cap, dst_slot, rights::READ) {
        0 => println!("CAP_TEST: Copy success"),
        e => {
            println!("CAP_TEST: Copy failed with error {}", e);
            return 1;
        }
    }
    
    // Test 2: Mint
    // Let's mint the same CNode to slot 101 with lower rights
    let dst_slot_mint = 101;
    println!("CAP_TEST: Testing CNode mint...");
    match cspace.mint(src_cap, dst_slot_mint, 0, rights::NONE) {
        0 => println!("CAP_TEST: Mint success"),
        e => {
            println!("CAP_TEST: Mint failed with error {}", e);
            return 1;
        }
    }

    // Test 3: Delete
    println!("CAP_TEST: Testing CNode delete...");
    match cspace.delete(dst_slot) {
        0 => println!("CAP_TEST: Delete success"),
        e => {
            println!("CAP_TEST: Delete failed with error {}", e);
            return 1;
        }
    }

    println!("CAP_TEST: All passed!");
    0
}
