#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec::Vec;
use glenda::println;

#[unsafe(no_mangle)]
fn main() -> usize {
    println!("HEAP_TEST: Starting...");

    let mut v = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    
    for i in 0..100 {
        assert_eq!(v[i], i);
    }
    println!("HEAP_TEST: Vec test passed");

    let b = alloc::boxed::Box::new(12345);
    assert_eq!(*b, 12345);
    println!("HEAP_TEST: Box test passed");

    println!("HEAP_TEST: All passed!");
    0
}
