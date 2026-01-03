#![no_std]
#![no_main]
use glenda as _;
use glenda::bootinfo::CONSOLE_CAP;
use glenda::cap::CapPtr;
use glenda::console;
use glenda::println;

#[unsafe(no_mangle)]
fn main() -> ! {
    // Initialize logging
    console::init(CapPtr(CONSOLE_CAP));
    println!("Hello World!");
    loop {}
}
