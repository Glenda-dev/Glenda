#![no_std]
#![no_main]
use glenda;
use glenda::println;
use glenda::runtime::KERNEL_CAP;

#[unsafe(no_mangle)]
fn main() -> usize {
    KERNEL_CAP.shell();
    println!("Hello World!");
    0
}
