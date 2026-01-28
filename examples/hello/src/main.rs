#![no_std]
#![no_main]
use glenda;
use glenda::println;

#[unsafe(no_mangle)]
fn main() -> usize {
    println!("Hello World!");
    0
}
