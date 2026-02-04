#![allow(dead_code)]
#![allow(unused)]

pub mod boot;
pub mod console;
pub mod cpu;
pub mod irq;
pub mod mem;
pub mod platform;
pub mod proc;
pub mod runtime;
pub mod trap;

pub const ARCH: &'static str = "none";
