mod buffer;
mod cmd;

use crate::hal;
use crate::printk;
use buffer::ShellBuffer;

pub fn run() {
    let mut buffer = ShellBuffer::new();
    loop {
        print_prompt();

        loop {
            let c = hal::console::read();

            match c {
                b'\r' | b'\n' => {
                    printk!("\n");
                    // Execute command returns true if we should exit the shell (e.g. boot command)
                    if cmd::execute_command(buffer.as_str()) {
                        return;
                    }
                    buffer.clear();
                    break;
                }
                0x08 | 0x7F => {
                    // Backspace
                    if buffer.len() > 0 {
                        buffer.pop();
                        printk!("\x08 \x08");
                    }
                }
                c => {
                    buffer.push(c);
                    printk!("{}", c as char);
                }
            }
        }
    }
}

fn print_prompt() {
    printk!("glenda> ");
}
