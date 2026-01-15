#![no_std]
#![no_main]
use glenda as _;
use glenda::println;

#[unsafe(no_mangle)]
fn main() -> ! {
    println!("Hello World!");
    loop {}
}
