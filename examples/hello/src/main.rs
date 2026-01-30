#![no_std]
#![no_main]
use glenda;
use glenda::cap::CSPACE_CAP;
use glenda::println;

#[unsafe(no_mangle)]
fn main() -> usize {
    CSPACE_CAP.debug_print();
    println!("Hello World!");
    0
}
